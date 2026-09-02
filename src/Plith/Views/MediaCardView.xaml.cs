using System.Windows.Controls;
using Plith.Cards;
using Plith.ViewModels;

namespace Plith.Views;

public partial class MediaCardView : UserControl
{
    public MediaCardView()
    {
        InitializeComponent();
        PrevButton.Click += (_, _) => Request(MediaCommand.SkipPrevious);
        PlayPauseButton.Click += (_, _) => Request(MediaCommand.TogglePlayPause);
        NextButton.Click += (_, _) => Request(MediaCommand.SkipNext);
    }

    // The DataContext is supplied implicitly by the DataTemplate that resolves this view from
    // MediaViewModel — this class never assigns it. The view-model is the only channel out,
    // so a null DataContext (designer, or a not-yet-bound container) is a silent no-op.
    private void Request(MediaCommand command) => (DataContext as MediaViewModel)?.RequestCommand(command);
}
