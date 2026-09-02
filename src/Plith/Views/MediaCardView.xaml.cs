using System.Windows.Controls;
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

    // The DataContext is supplied by whoever hosts this view — an explicit assignment today,
    // an implicit DataTemplate from Task 6 on. Either way the view-model is the only channel
    // out, so a null DataContext (designer, or a not-yet-bound container) is a silent no-op.
    private void Request(MediaCommand command) => (DataContext as MediaViewModel)?.RequestCommand(command);
}

public enum MediaCommand { SkipPrevious, TogglePlayPause, SkipNext }
