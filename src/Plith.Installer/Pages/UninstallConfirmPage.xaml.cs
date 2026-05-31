using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;

namespace Plith.Installer.Pages;

public partial class UninstallConfirmPage : UserControl
{
    public event EventHandler? UninstallClicked;

    public UninstallConfirmPage()
    {
        InitializeComponent();

        UninstallButton.Click += (_, _) => UninstallClicked?.Invoke(this, EventArgs.Empty);
        CancelButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
