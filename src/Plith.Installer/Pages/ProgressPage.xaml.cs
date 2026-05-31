using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class ProgressPage : UserControl
{
    public ProgressPage(InstallerViewModel vm, string headline)
    {
        InitializeComponent();
        DataContext = vm;
        HeadlineText.Text = headline;

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
