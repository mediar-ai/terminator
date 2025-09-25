use crate::Selector;

#[test]
fn parses_or_with_commas() {
    let sel = Selector::from("role:button, name:Submit");
    match sel {
        Selector::Or(v) => {
            assert_eq!(v.len(), 2);
        }
        _ => panic!("expected Or"),
    }
}

#[test]
fn parses_and_with_double_ampersand() {
    let sel = Selector::from("role:button && name:Submit");
    match sel {
        Selector::And(v) => {
            assert_eq!(v.len(), 2);
        }
        _ => panic!("expected And"),
    }
}

#[test]
fn chain_with_and_segment() {
    let sel = Selector::from("application:Notepad >> role:button && name:OK");
    match sel {
        Selector::Chain(parts) => {
            assert_eq!(parts.len(), 2);
            match &parts[1] {
                Selector::And(v) => assert_eq!(v.len(), 2),
                other => panic!("expected And, got {:?}", other),
            }
        }
        _ => panic!("expected Chain"),
    }
}
