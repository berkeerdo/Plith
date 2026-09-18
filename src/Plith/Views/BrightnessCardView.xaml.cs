using System.Windows.Controls;
using Plith.ViewModels;

namespace Plith.Views;

public partial class BrightnessCardView : UserControl
{
    public BrightnessCardView()
    {
        InitializeComponent();
        LiveRegionAnnouncer.Attach(this, nameof(BrightnessCardViewModel.AccessibleSummary));
    }
}
