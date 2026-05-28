using System.Globalization;
using System.Windows.Data;

namespace Plith.ViewModels;

public sealed class NormalizedToWidthConverter : IMultiValueConverter
{
    public object Convert(object[] values, Type targetType, object? parameter, CultureInfo culture)
    {
        if (values.Length < 2) return 0d;
        var normalized = values[0] is double n ? n : 0d;
        var fullWidth = values[1] is double w ? w : 0d;
        return Math.Max(0d, Math.Min(fullWidth, fullWidth * normalized));
    }

    public object[] ConvertBack(object value, Type[] targetTypes, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
