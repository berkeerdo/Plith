using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace Plith.Installer.ViewModels;

/// <summary>Shows the bound element only when InstallStepViewModel.Status matches the
/// parameter (e.g. ConverterParameter=Running → Visible only while step is running).</summary>
public sealed class StatusToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is InstallStepStatus status && parameter is string target
            && Enum.TryParse<InstallStepStatus>(target, out var targetStatus))
        {
            return status == targetStatus ? Visibility.Visible : Visibility.Collapsed;
        }
        return Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
