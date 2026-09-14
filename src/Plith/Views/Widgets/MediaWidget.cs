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

    public MediaWidget(MediaViewModel vm)
    {
        ArgumentNullException.ThrowIfNull(vm);
        InitializeComponent();
        _vm = vm;

        _vm.PropertyChanged += (_, _) => Render();

        Previous.Click += (_, _) => _vm.RequestCommand(MediaCommand.SkipPrevious);
        Next.Click += (_, _) => _vm.RequestCommand(MediaCommand.SkipNext);
        PlayPause.Click += (_, _) => _vm.RequestCommand(MediaCommand.TogglePlayPause);

        // Play/pause sits a touch brighter than the two beside it, as the design has it: it is
        // the one a person reaches for without looking.
        PlayPause.Background = new SolidColorBrush(Color.FromArgb(0x1C, 0xFF, 0xFF, 0xFF));

        // Re-rendered on the way in as well as on change: a page that has been away misses every
        // notification while it is off the tree, so arriving without this would show whatever
        // was playing when it last left.
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
    }
}
