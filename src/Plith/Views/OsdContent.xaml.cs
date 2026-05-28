using System.Windows.Controls;

namespace Plith.Views;

public partial class OsdContent : UserControl
{
    public event EventHandler<MediaCommand>? MediaCommandInvoked;

    public OsdContent()
    {
        InitializeComponent();
        MediaCardControl.CommandInvoked += (s, cmd) => MediaCommandInvoked?.Invoke(this, cmd);
    }
}
