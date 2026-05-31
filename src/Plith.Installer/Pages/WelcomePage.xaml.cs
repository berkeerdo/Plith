using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class WelcomePage : UserControl
{
    public event EventHandler? PrimaryClicked;

    public WelcomePage(InstallerViewModel vm)
    {
        InitializeComponent();
        DataContext = vm;

        PrimaryButton.Click += (_, _) => PrimaryClicked?.Invoke(this, EventArgs.Empty);

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb)
                sb.Begin(this);
        };
    }
}
