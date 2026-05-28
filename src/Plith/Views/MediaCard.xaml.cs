using System.Windows;
using System.Windows.Controls;

namespace Plith.Views;

public partial class MediaCard : UserControl
{
    /// <summary>Raised when a transport button is clicked. Owner runs the command on the SMTC session.</summary>
    public event EventHandler<MediaCommand>? CommandInvoked;

    public MediaCard()
    {
        InitializeComponent();
        PrevButton.Click += (_, _) => CommandInvoked?.Invoke(this, MediaCommand.SkipPrevious);
        PlayPauseButton.Click += (_, _) => CommandInvoked?.Invoke(this, MediaCommand.TogglePlayPause);
        NextButton.Click += (_, _) => CommandInvoked?.Invoke(this, MediaCommand.SkipNext);
    }
}

public enum MediaCommand { SkipPrevious, TogglePlayPause, SkipNext }
