using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;

namespace Plith.Installer.Pages;

public partial class UninstallFinishPage : UserControl
{
    public UninstallFinishPage()
    {
        InitializeComponent();
        CloseButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
