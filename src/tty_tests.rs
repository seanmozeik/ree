use super::*;

#[test]
fn repairs_input_modes() {
    let removed = InputModes::IGNBRK
        | InputModes::PARMRK
        | InputModes::INPCK
        | InputModes::ISTRIP
        | InputModes::INLCR
        | InputModes::IGNCR
        | InputModes::IXOFF
        | InputModes::IXANY;
    let required = InputModes::BRKINT
        | InputModes::IGNPAR
        | InputModes::ICRNL
        | InputModes::IXON
        | InputModes::IMAXBEL;
    let mut modes = removed;

    repair_input_modes(&mut modes);

    assert!(!modes.intersects(removed));
    assert!(modes.contains(required));
}

#[test]
fn repairs_output_modes() {
    let removed = OutputModes::OCRNL | OutputModes::ONOCR | OutputModes::ONLRET;
    let required = OutputModes::OPOST | OutputModes::ONLCR;
    let mut modes = OutputModes::all();

    repair_output_modes(&mut modes);

    assert!(!modes.intersects(removed));
    assert!(modes.contains(required));
}

#[test]
fn repairs_control_modes() {
    let removed =
        ControlModes::PARENB | ControlModes::PARODD | ControlModes::CSTOPB | ControlModes::CLOCAL;
    let required = ControlModes::CS8 | ControlModes::CREAD;
    let mut modes = ControlModes::all();

    repair_control_modes(&mut modes);

    assert!(!modes.intersects(removed));
    assert!(modes.contains(required));
    assert_eq!(modes & ControlModes::CSIZE, ControlModes::CS8);
}

#[test]
fn repairs_local_modes() {
    let removed = LocalModes::ECHONL
        | LocalModes::NOFLSH
        | LocalModes::TOSTOP
        | LocalModes::ECHOPRT
        | LocalModes::FLUSHO
        | LocalModes::PENDIN
        | LocalModes::EXTPROC;
    let required = LocalModes::ISIG
        | LocalModes::ICANON
        | LocalModes::IEXTEN
        | LocalModes::ECHO
        | LocalModes::ECHOE
        | LocalModes::ECHOK
        | LocalModes::ECHOKE
        | LocalModes::ECHOCTL;
    let mut modes = removed;

    repair_local_modes(&mut modes);

    assert!(!modes.intersects(removed));
    assert!(modes.contains(required));
}

#[test]
fn repairs_only_disabled_control_characters() {
    let mut zero = 0;
    repair_control_character_value(&mut zero, 0o4);
    assert_eq!(zero, 0o4);

    let mut disabled = POSIX_VDISABLE;
    repair_control_character_value(&mut disabled, 0o3);
    assert_eq!(disabled, 0o3);

    let mut customized = 0o10;
    repair_control_character_value(&mut customized, 0o177);
    assert_eq!(customized, 0o10);
}
