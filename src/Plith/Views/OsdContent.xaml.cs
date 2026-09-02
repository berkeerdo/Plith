using System.Windows.Controls;
using Plith.ViewModels;

namespace Plith.Views;

public partial class OsdContent : UserControl
{
    public event EventHandler<MediaCommand>? MediaCommandInvoked;

    public OsdContent()
    {
        InitializeComponent();
        DataContextChanged += (_, e) =>
        {
            if (e.OldValue is OsdViewModel old) old.Media.CommandRequested -= OnCommandRequested;
            if (e.NewValue is OsdViewModel now) now.Media.CommandRequested += OnCommandRequested;
        };
    }

    private void OnCommandRequested(MediaCommand command) => MediaCommandInvoked?.Invoke(this, command);
}
