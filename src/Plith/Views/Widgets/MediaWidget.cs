using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
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

        // Re-rendered on the way in as well as on change: a page that has been away misses every
        // notification while it is off the tree, so arriving without this would show whatever
        // was playing when it last left.
        IsVisibleChanged += (_, e) => { if ((bool)e.NewValue) Render(); };
        Render();
    }

    private void Render()
    {
        Title.Text = _vm.HasSession ? _vm.Title : "Nothing playing";
        Artist.Text = _vm.HasSession ? _vm.Artist : string.Empty;
        Art.Source = _vm.AlbumArt;

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
