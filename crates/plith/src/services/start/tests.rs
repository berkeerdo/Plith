use super::*;

#[test]
fn the_board_tracks_finished_starts() {
    let board = StartBoard::default();
    assert!(!board.all_up());
    board.mark(ServiceId::Media);
    board.mark(ServiceId::Voicemeeter);
    board.mark(ServiceId::Media);
    assert_eq!(board.up(), [ServiceId::Voicemeeter, ServiceId::Media]);
    for id in ServiceId::ALL {
        board.mark(id);
    }
    assert!(board.all_up());
    assert_eq!(board.up(), ServiceId::ALL);
}
