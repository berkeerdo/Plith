namespace Plith.Cards;

/// <summary>Transport commands the media card's view can request. Lives in Plith.Cards (not
/// Plith.Views) because both MediaViewModel (Plith.ViewModels) and MediaCard (Plith.Cards)
/// depend on it — putting it in Views would make those layers depend on the view layer, which
/// inverts MVVM.</summary>
public enum MediaCommand { SkipPrevious, TogglePlayPause, SkipNext }
