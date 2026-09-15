using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Animation;
using Plith.Cards;
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

        _vm.PropertyChanged += (_, _) => Render();

        Previous.Click += (_, _) => _vm.RequestCommand(MediaCommand.SkipPrevious);
        Next.Click += (_, _) => _vm.RequestCommand(MediaCommand.SkipNext);
        PlayPause.Click += (_, _) => _vm.RequestCommand(MediaCommand.TogglePlayPause);

        // Play/pause sits a touch brighter than the two beside it, as the design has it: it is
        // the one a person reaches for without looking.
        // Play/pause is the one a hand goes to without looking, so it is filled rather than
        // merely a shade brighter - the difference between "three controls, one of them slightly
        // different" and "a control, with two beside it".
        PlayPause.Background = new SolidColorBrush(Color.FromArgb(0x2E, 0xFF, 0xFF, 0xFF));
        PlayPause.Margin = new Thickness(6, 0, 6, 0);

        // Re-rendered on the way in as well as on change: a page that has been away misses every
        // notification while it is off the tree, so arriving without this would show whatever
        // was playing when it last left.
        // The backdrop reaches the frame's edges, so the page takes the notch's outline the way
        // the weather page does - otherwise the artwork draws square corners inside a rounded
        // shape and overhangs into nothing.
        SizeChanged += (_, e) => Clip = Presentation.NotchGeometry.BottomRoundedClip(e.NewSize);

        IsVisibleChanged += (_, e) => { if ((bool)e.NewValue) Render(); };
        Render();
    }

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
    /// <summary>
    /// The album-coloured backdrop.
    ///
    /// Its own method because it wants an early return, and the one it had was inside Render -
    /// so whenever the backdrop was already correct, Render stopped there and never reached the
    /// play/pause shape or the transport's enabled state. The pause mark simply vanished. An
    /// early return is only safe in a method that does one thing.
    /// </summary>
    private void RenderBackdrop()
    {
        Backdrop.Source = _vm.AlbumArt;
        var wantBackdrop = _vm.AlbumArt is not null ? BackdropOpacity : 0.0;
        if (Math.Abs(Backdrop.Opacity - wantBackdrop) <= 0.01) return;

        // Animated only when there is a clock to animate against. Off screen — and in any host
        // that lays the control out without presenting it — a DoubleAnimation never ticks, so
        // the backdrop would sit at its starting value forever and the page would render without
        // the artwork it is built around. Assigning covers that; the fade is the nicety.
        if (!IsVisible || !SystemParameters.ClientAreaAnimation)
        {
            Backdrop.BeginAnimation(OpacityProperty, null);
            Backdrop.Opacity = wantBackdrop;
            return;
        }

        Backdrop.BeginAnimation(OpacityProperty,
            new DoubleAnimation(wantBackdrop, TimeSpan.FromMilliseconds(320))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            });

    }

    /// <summary>
    /// How strongly the artwork shows through.
    ///
    /// Low on purpose. The backdrop is there to give the page the album's colour, not to be
    /// looked at: past about a third the scrim stops winning and the title starts competing with
    /// whatever happens to be in the picture behind it.
    /// </summary>
    private const double BackdropOpacity = 0.32;

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

        RenderBackdrop();

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
    }
}
