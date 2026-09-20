using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using Plith.Cards;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Views.Widgets;

/// <summary>
/// What is playing, and the controls to do something about it.
///
/// Reads the same <see cref="MediaViewModel"/> the Classic card's media row does, rather than a
/// second one: two view models for one session would eventually disagree about whether it is
/// playing, and the two surfaces are visible seconds apart.
/// </summary>
public partial class MediaWidget : UserControl
{
    private readonly MediaViewModel _vm;

    /// <param name="openSource">Brings the app that owns the session to the front. Null leaves
    /// the art and title inert rather than looking pressable and doing nothing.</param>
    public MediaWidget(MediaViewModel vm, Action? openSource = null)
    {
        ArgumentNullException.ThrowIfNull(vm);
        InitializeComponent();
        _vm = vm;

        if (openSource is not null)
        {
            // The art and the text, not the whole page: the transport sits on the same row, and
            // a page-wide click target would swallow every press meant for a transport button.
            OpenSourceArea.Cursor = System.Windows.Input.Cursors.Hand;
            OpenSourceArea.MouseLeftButtonUp += (_, e) => { openSource(); e.Handled = true; };
            System.Windows.Automation.AutomationProperties.SetName(OpenSourceArea, "Open the app that is playing");
        }

        _vm.PropertyChanged += (_, e) =>
        {
            // The position arrives about once a second on some sources. A full Render reassigns
            // the artwork and both marquees, so it goes straight to the progress row instead.
            if (e.PropertyName == nameof(MediaViewModel.Timeline)) RenderProgress();
            else Render();
        };

        Previous.Click += (_, _) => _vm.RequestCommand(MediaCommand.SkipPrevious);
        Next.Click += (_, _) => _vm.RequestCommand(MediaCommand.SkipNext);
        PlayPause.Click += (_, _) => _vm.RequestCommand(MediaCommand.TogglePlayPause);

        // Windows' own sound page, not a device list of ours. Changing the default
        // endpoint has no documented API, only the undocumented IPolicyConfig, so an
        // in-notch picker is its own slice. See SystemSoundPanel.
        Output.Click += (_, _) => SystemSoundPanel.TryOpen();

        // The track writes on RELEASE, not on every sample of the drag. Writing continuously
        // would send SMTC a position write per mouse move, which makes the source scrub and
        // stutter; Spotify's own bar behaves the same way. DragStarted and DragCompleted cover a
        // click as well as a drag, because IsMoveToPointEnabled turns a click into a drag of the
        // thumb it just moved.
        Bar.AddHandler(Thumb.DragStartedEvent,
            new DragStartedEventHandler((_, _) => _dragging = true));
        Bar.AddHandler(Thumb.DragCompletedEvent,
            new DragCompletedEventHandler((_, _) => { _dragging = false; CommitSeek(); }));
        Bar.ValueChanged += OnBarValueChanged;

        // Play/pause sits a touch brighter than the two beside it, as the design has it: it is
        // the one a person reaches for without looking.
        // Play/pause is the one a hand goes to without looking, so it is filled rather than
        // merely a shade brighter - the difference between "three controls, one of them slightly
        // different" and "a control, with two beside it".
        PlayPause.Background = new SolidColorBrush(Color.FromArgb(0x2E, 0xFF, 0xFF, 0xFF));
        PlayPause.Margin = new Thickness(6, 0, 6, 0);

        _tick = new DispatcherTimer(DispatcherPriority.Background, Dispatcher)
        {
            Interval = TimeSpan.FromSeconds(1),
        };
        _tick.Tick += (_, _) => RenderProgress();

        // Re-rendered on the way in as well as on change: a page that has been away misses every
        // notification while it is off the tree, so arriving without this would show whatever was
        // playing when it last left.
        IsVisibleChanged += (_, e) =>
        {
            if ((bool)e.NewValue) { Render(); _tick.Start(); }
            else _tick.Stop();
        };

        Render();
    }

    /// <summary>
    /// Moves the bar between readings.
    ///
    /// 1 Hz is enough for both halves of the row: the seconds text changes at 1 Hz, and a 232 DIP
    /// bar over a four minute track advances about one DIP per second.
    ///
    /// Started and stopped with visibility rather than left running. A page that is off the tree
    /// is not being looked at, and this widget is built once and kept for the life of the window,
    /// so a timer left running would tick for the whole session to paint nothing.
    /// </summary>
    private readonly DispatcherTimer _tick;

    /// <summary>True between the thumb being grabbed and released.</summary>
    private bool _dragging;

    /// <summary>
    /// When the user last drove the track, so the tick does not pull the thumb out from under
    /// their hand.
    ///
    /// A flag beside a timestamp rather than a sentinel timestamp alone, and that is AudioWidget's
    /// recorded lesson rather than a preference: initialising the stamp to long.MinValue and
    /// asking whether TickCount64 minus it is small OVERFLOWS to a negative number, which is
    /// smaller than the window, so the widget believes the user is driving from the moment it is
    /// built and never paints the position at all. That defect was found by rendering the widget
    /// offscreen, with the track at zero beside a readout that said 62 per cent.
    /// </summary>
    private long _lastUserChangeMs;

    private bool _hasUserChanged;

    /// <summary>
    /// How long after a user-driven change the tick stays out of the way.
    ///
    /// Long enough to cover the round trip: the seek is written on release, the source applies it
    /// and reports a new timeline back, and until that arrives the interpolation is still running
    /// from the OLD reading. Writing the bar from it in that gap is what would snap the thumb back
    /// to where the track was before the gesture.
    /// </summary>
    private const long UserDrivingWindowMs = 900;

    private bool UserIsDriving
        => _hasUserChanged && Environment.TickCount64 - _lastUserChangeMs < UserDrivingWindowMs;

    /// <summary>What the page last showed, so a repaint that changes nothing does not animate.
    /// Every property change on the view model lands here — play/pause alone must not make the
    /// title jump.</summary>
    private string? _shownTitle;

    /// <summary>
    /// A new track arrives rather than appearing.
    ///
    /// Short and small on purpose: 220 ms and eight DIP. The page is 356 wide and a person is
    /// already looking at it, so anything longer reads as waiting and anything larger reads as a
    /// page turn — which is what the pager does, and this is not that.
    ///
    /// The art cross-fades in place while the text slides, because the two carry different
    /// things: the art is the album, which simply becomes another album, and the text is the
    /// thing you are reading, which is replaced.
    /// </summary>
    private void AnimateTrackChange()
    {
        if (!SystemParameters.ClientAreaAnimation) return;

        var ease = new CubicEase { EasingMode = EasingMode.EaseOut };
        var duration = TimeSpan.FromMilliseconds(220);

        var shift = new TranslateTransform();
        TextColumn.RenderTransform = shift;
        shift.BeginAnimation(TranslateTransform.XProperty,
            new DoubleAnimation(8, 0, duration) { EasingFunction = ease });
        TextColumn.BeginAnimation(OpacityProperty,
            new DoubleAnimation(0, 1, duration) { EasingFunction = ease });

        ArtHost.BeginAnimation(OpacityProperty,
            new DoubleAnimation(0.35, 1, duration) { EasingFunction = ease });
    }

    private void Render()
    {
        var title = _vm.HasSession ? _vm.Title : "Nothing playing";
        var trackChanged = _shownTitle is not null && !string.Equals(_shownTitle, title, StringComparison.Ordinal);
        _shownTitle = title;

        Title.Text = title;
        Artist.Text = _vm.HasSession ? _vm.Artist : string.Empty;
        Art.Source = _vm.AlbumArt;

        if (trackChanged) AnimateTrackChange();

        // The play/pause glyph is chosen here rather than bound to the view model's
        // PlayPauseGlyph, which is a Segoe MDL2 code point. This surface draws its own icons -
        // see PlithIcons.xaml for why depending on that font is a portability problem and not a
        // stylistic preference.
        PlayPauseGlyph.Data = (Geometry)FindResource(_vm.IsPlaying ? "IconPause" : "IconPlay");

        // The name changes with the state, because "Play" on a button that pauses is worse than
        // no name at all. Set on the Button, which has a peer - a name on the Path inside it
        // would reach nothing.
        AutomationProperties.SetName(PlayPause, _vm.PlayPauseLabel);

        // The transport is pointless with no session, and a control that does nothing when
        // pressed teaches people not to trust the ones that do.
        var enabled = _vm.HasSession;
        Previous.IsEnabled = enabled;
        Next.IsEnabled = enabled;
        PlayPause.IsEnabled = enabled;

        RenderProgress();
    }

    /// <summary>
    /// Where the track is, in the bar and in the two clocks.
    ///
    /// Its own method, and the one early return in this file lives here rather than in Render.
    /// That is not a style choice: Render's early return used to sit in the middle of it, so
    /// whenever the backdrop was already correct it stopped there and never reached the
    /// play/pause shape. An early return is only safe in a method that does one thing.
    ///
    /// A null timeline collapses the row rather than drawing an empty bar. A source with no end
    /// time (a live stream) reports null, and a bar of unknown length is a lie.
    /// </summary>
    private void RenderProgress()
    {
        var timeline = _vm.Timeline;
        if (timeline is null || !_vm.HasSession)
        {
            ProgressRow.Visibility = Visibility.Collapsed;
            return;
        }

        ProgressRow.Visibility = Visibility.Visible;

        PaintBar();

        // Only a source that accepts a position write gets a draggable track. The bar still
        // reports where the track is either way.
        Bar.IsEnabled = _vm.CanSeek;

        // The hand wins. While the thumb is held, or inside the window after a write, the bar is
        // the user's and the interpolation is still running from a reading that predates their
        // gesture: writing it here is what would snap the thumb back.
        if (_dragging || UserIsDriving)
        {
            ShowTimesFor(TargetPosition());
            return;
        }

        var elapsed = MediaProgress.Elapsed(timeline.Position, timeline.LastUpdated,
                                            timeline.Duration, _vm.IsPlaying, DateTimeOffset.Now);

        // Detached around the write, so this repaint cannot come back through OnBarValueChanged
        // and turn a report into a seek.
        Bar.ValueChanged -= OnBarValueChanged;
        // Duration is positive by construction: ReadTimeline returns null otherwise, which is
        // what makes this division safe without a guard here.
        Bar.Value = elapsed / timeline.Duration * 100;
        Bar.ValueChanged += OnBarValueChanged;

        ShowTimesFor(elapsed);
    }

    private void ShowTimesFor(TimeSpan elapsed)
    {
        var duration = _vm.Timeline?.Duration ?? TimeSpan.Zero;
        Elapsed.Text = MediaProgress.Clock(elapsed);
        Remaining.Text = "-" + MediaProgress.Clock(duration - elapsed);
    }

    /// <summary>
    /// The track moved, and this only ever runs for a change the USER made.
    ///
    /// RenderProgress detaches this handler around its own write, which is the same shape
    /// AudioWidget uses and for the same reason: otherwise a repaint reads as a gesture and the
    /// page seeks the source to wherever it had just finished drawing.
    /// </summary>
    private void OnBarValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        _hasUserChanged = true;
        _lastUserChangeMs = Environment.TickCount64;

        // Labels follow the thumb while it is held, so the gesture has a readout. The position
        // itself is not written until release.
        ShowTimesFor(TargetPosition());

        // Keyboard: arrows, Home and End raise this with no drag around it, so there is no
        // DragCompleted coming to commit it.
        if (!_dragging) CommitSeek();
    }

    /// <summary>Where the thumb currently points, in the track's own time.</summary>
    private TimeSpan TargetPosition()
    {
        var duration = _vm.Timeline?.Duration ?? TimeSpan.Zero;
        return MediaProgress.PositionFor(Bar.Value / Bar.Maximum, duration);
    }

    private void CommitSeek()
    {
        if (!_vm.CanSeek || _vm.Timeline is null) return;

        _lastUserChangeMs = Environment.TickCount64;   // the round trip starts now, not at the grab
        _vm.RequestSeek(TargetPosition());
    }

    /// <summary>How much of the ink the unplayed groove keeps.</summary>
    private const byte GrooveAlpha = 0x3D;

    /// <summary>
    /// The bar's two colours, computed from the page's ink.
    ///
    /// The played part is the ink itself and the groove is the same ink at
    /// <see cref="GrooveAlpha"/>, which is the one arrangement that cannot come out backwards.
    /// Three others were tried and measured first:
    ///
    /// The accent on NotchTrack failed check-contrast.ps1 at 1,0:1 with a lime accent on the
    /// light theme, because the user's accent can land anywhere including on the track.
    ///
    /// NotchInk on NotchTrack reached only 2,6:1 with a near-white accent, because
    /// ContrastInk.TrackOn walks from the surface just far enough to clear 3:1 AGAINST THE
    /// SURFACE and stops, leaving the ink an unpredictable distance further along the same ramp.
    ///
    /// An ink derived from the groove with ContrastInk.PairOn cleared the ratio and drew the bar
    /// INVERTED: on a dark-theme surface TrackOn returns a light grey, so the derived ink is
    /// near-black and the played part read as a hole punched in the groove. Found in
    /// widget-media.png, and not visible to the contrast lint, which measures ratios and has no
    /// notion of which side should be stronger.
    ///
    /// Computed on every repaint rather than in the constructor: a brush written once cannot
    /// follow a theme or accent change, which is the staleness this branch has produced five
    /// times over.
    /// </summary>
    private void PaintBar()
    {
        if (FindResource("NotchInk") is not SolidColorBrush ink) return;

        Bar.Foreground = ink;
        Bar.Background = new SolidColorBrush(
            Color.FromArgb(GrooveAlpha, ink.Color.R, ink.Color.G, ink.Color.B));
    }
}
