//! Historia zmian projektu: cofanie, ponawianie i skok do dowolnego kroku.
//!
//! Każdy krok to pełny stan tego, co użytkownik edytuje: krzywe, operacje,
//! półfabrykat i ustawienia G-code. Zmiana jest wykrywana przez porównanie
//! dokumentu z ostatnim zapisanym krokiem (krzywe — po liczniku zmian), więc
//! historia łapie każdą edycję bez wpisywania jej w każdym miejscu programu.
//! Krzywe są współdzielone między krokami, w których się nie zmieniły.

use crate::doc::{DocContour, Document, Stock};
use crate::icons::Icon;
use crate::toolbar::Sub;
use cam_ops::Operation;
use cam_post::PostSettings;
use std::rc::Rc;

/// Najwięcej kroków w historii; najstarsze są usuwane.
const MAX_STEPS: usize = 200;

/// Znaczek w rogu ikony kroku: co się stało.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Badge {
    None,
    Add,
    Remove,
    Edit,
    Curves,
    Order,
    On,
    Off,
}

/// Wygląd kroku w panelu historii: ikona, znaczek i operacja (jej kolor).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Mark {
    pub icon: Icon,
    pub badge: Badge,
    pub op: Option<usize>,
}

impl Mark {
    pub fn new(icon: Icon) -> Self {
        Mark { icon, badge: Badge::None, op: None }
    }

    pub fn badge(mut self, b: Badge) -> Self {
        self.badge = b;
        self
    }

    fn op(i: usize, op: &Operation, b: Badge) -> Self {
        Mark { icon: Sub::for_op(&op.kind).icon(), badge: b, op: Some(i) }
    }
}

struct Step {
    label: String,
    mark: Mark,
    contours: Rc<Vec<DocContour>>,
    /// `geom_revision` dokumentu, przy którym zapisano krzywe.
    geom: u64,
    ops: Vec<Operation>,
    stock: Stock,
    post: PostSettings,
}

pub struct History {
    steps: Vec<Step>,
    /// Krok, w którym jest teraz dokument.
    pos: usize,
    /// Zmienia się przy każdej zmianie historii (panel przewija się do bieżącego kroku).
    pub serial: u64,
}

impl History {
    pub fn new(doc: &Document, label: impl Into<String>) -> Self {
        let mut h = History { steps: Vec::new(), pos: 0, serial: 0 };
        h.reset(doc, label);
        h
    }

    /// Nowa historia od bieżącego stanu (po otwarciu pliku).
    pub fn reset(&mut self, doc: &Document, label: impl Into<String>) {
        let contours = Rc::new(doc.contours.clone());
        self.steps = vec![step(doc, (Mark::new(Icon::Open), label.into()), contours)];
        self.pos = 0;
        self.serial += 1;
    }

    fn current(&self) -> &Step {
        &self.steps[self.pos]
    }

    /// Czy dokument różni się od bieżącego kroku.
    fn changed(&self, doc: &Document) -> bool {
        let c = self.current();
        doc.geom_revision != c.geom || doc.ops != c.ops || doc.stock != c.stock || doc.post != c.post
    }

    /// Wywoływane co klatkę. Zapisuje zmianę jako nowy krok, gdy edycja się
    /// skończyła (`settled`: puszczony przycisk myszy, żadne pole nie jest
    /// edytowane) — przeciąganie suwaka czy krzywej daje jeden krok.
    pub fn track(&mut self, doc: &mut Document, settled: bool) {
        if !self.changed(doc) {
            doc.edit_label = None;
            return;
        }
        if settled {
            self.commit(doc);
        }
    }

    fn commit(&mut self, doc: &mut Document) {
        let label = doc.edit_label.take().unwrap_or_else(|| describe(self.current(), doc));
        let contours = if doc.geom_revision == self.current().geom {
            self.current().contours.clone()
        } else {
            Rc::new(doc.contours.clone())
        };
        self.steps.truncate(self.pos + 1);
        self.steps.push(step(doc, label, contours));
        if self.steps.len() > MAX_STEPS {
            self.steps.remove(0);
        }
        self.pos = self.steps.len() - 1;
        self.serial += 1;
    }

    pub fn can_undo(&self) -> bool {
        self.pos > 0
    }

    pub fn can_redo(&self) -> bool {
        self.pos + 1 < self.steps.len()
    }

    /// Cofa jeden krok. Zwraca opis cofniętej zmiany.
    pub fn undo(&mut self, doc: &mut Document) -> Option<String> {
        if self.changed(doc) {
            self.commit(doc); // niezapisana jeszcze zmiana też jest krokiem
        }
        let label = self.current().label.clone();
        self.jump(doc, self.pos.checked_sub(1)?).then_some(label)
    }

    /// Ponawia jeden krok. Zwraca opis przywróconej zmiany.
    pub fn redo(&mut self, doc: &mut Document) -> Option<String> {
        if self.changed(doc) {
            self.commit(doc);
            return None; // po nowej zmianie nie ma czego ponawiać
        }
        let next = self.pos + 1;
        self.jump(doc, next).then(|| self.current().label.clone())
    }

    /// Przywraca dokument do kroku `i`.
    pub fn jump(&mut self, doc: &mut Document, i: usize) -> bool {
        if i >= self.steps.len() || (i == self.pos && !self.changed(doc)) {
            return false;
        }
        if self.changed(doc) {
            self.commit(doc);
        }
        let s = &self.steps[i];
        doc.restore_state((*s.contours).clone(), s.ops.clone(), s.stock.clone(), s.post.clone());
        self.pos = i;
        self.steps[i].geom = doc.geom_revision;
        self.serial += 1;
        true
    }

    /// Kroki do wyświetlenia: (numer, opis, wygląd, czy bieżący, czy cofnięty).
    pub fn entries(&self) -> impl Iterator<Item = (usize, &str, Mark, bool, bool)> {
        self.steps.iter().enumerate().map(|(i, s)| (i, s.label.as_str(), s.mark, i == self.pos, i > self.pos))
    }
}

fn step(doc: &Document, (mark, label): (Mark, String), contours: Rc<Vec<DocContour>>) -> Step {
    Step {
        label,
        mark,
        contours,
        geom: doc.geom_revision,
        ops: doc.ops.clone(),
        stock: doc.stock.clone(),
        post: doc.post.clone(),
    }
}

/// Nazwa i ikona zmiany, gdy kod, który ją zrobił, nie podał opisu.
fn describe(prev: &Step, doc: &Document) -> (Mark, String) {
    if doc.geom_revision != prev.geom {
        return (Mark::new(Icon::Curve).badge(Badge::Edit), "Zmiana krzywych".into());
    }
    // dodana / usunięta: pierwsze miejsce, w którym listy się różnią (kopia operacji też się liczy)
    if doc.ops.len() > prev.ops.len() {
        let i = (0..doc.ops.len()).find(|&i| prev.ops.get(i) != Some(&doc.ops[i])).unwrap_or(doc.ops.len() - 1);
        let op = &doc.ops[i];
        return (Mark::op(i, op, Badge::Add), format!("Nowa operacja: {}", op.name));
    }
    if doc.ops.len() < prev.ops.len() {
        let i = (0..prev.ops.len()).find(|&i| doc.ops.get(i) != Some(&prev.ops[i])).unwrap_or(prev.ops.len() - 1);
        let op = &prev.ops[i];
        return (Mark::op(i, op, Badge::Remove), format!("Usunięto operację: {}", op.name));
    }
    if doc.ops.len() == prev.ops.len() && doc.ops != prev.ops {
        let changed: Vec<usize> = (0..doc.ops.len()).filter(|&i| doc.ops[i] != prev.ops[i]).collect();
        if changed.len() == 2 && doc.ops[changed[0]] == prev.ops[changed[1]] && doc.ops[changed[1]] == prev.ops[changed[0]] {
            let i = changed[0];
            return (Mark::op(i, &doc.ops[i], Badge::Order), "Kolejność operacji".into());
        }
        if let [i] = changed[..] {
            let (a, b) = (&prev.ops[i], &doc.ops[i]);
            let (badge, what) = if a.contours != b.contours {
                (Badge::Curves, "krzywe")
            } else if a.name != b.name {
                return (Mark::op(i, b, Badge::Edit), format!("Nazwa operacji: {}", b.name));
            } else if a.enabled != b.enabled {
                if b.enabled { (Badge::On, "włączona") } else { (Badge::Off, "wyłączona") }
            } else if a.tool_id != b.tool_id {
                (Badge::Edit, "narzędzie")
            } else {
                (Badge::Edit, "parametry")
            };
            return (Mark::op(i, b, badge), format!("{}: {what}", b.name));
        }
        return (Mark::new(Icon::Document).badge(Badge::Edit), "Zmiana operacji".into());
    }
    if doc.stock != prev.stock {
        return (Mark::new(Icon::Stock).badge(Badge::Edit), "Półfabrykat".into());
    }
    if doc.post != prev.post {
        return (Mark::new(Icon::Document).badge(Badge::Edit), "Ustawienia G-code".into());
    }
    (Mark::new(Icon::Document), "Zmiana".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cam_geom::{Contour, Vec2};

    fn doc_with_square() -> Document {
        let mut d = Document::default();
        let mut sq = Contour::new(true);
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)] {
            sq.verts.push(cam_geom::Vertex::new(x, y, 0.0));
        }
        d.add_contours(&[(sq, String::new())], Vec2::new(0.0, 0.0));
        d.edit_label = None;
        d
    }

    #[test]
    fn undo_redo_move() {
        let mut d = doc_with_square();
        let mut h = History::new(&d, "Start");
        assert!(d.move_selected(Vec2::new(5.0, 0.0)));
        h.track(&mut d, false); // przeciąganie trwa — jeszcze bez kroku
        assert!(!h.can_undo());
        d.move_selected(Vec2::new(5.0, 0.0));
        h.track(&mut d, true);
        assert_eq!(d.contours[0].bbox.min.x, 10.0);
        assert_eq!(h.entries().count(), 2, "przeciąganie = jeden krok");

        assert_eq!(h.undo(&mut d).as_deref(), Some("Przesunięcie"));
        assert_eq!(d.contours[0].bbox.min.x, 0.0);
        h.track(&mut d, true);
        assert!(h.can_redo(), "cofnięcie nie może zostać zapisane jako nowa zmiana");
        assert!(h.redo(&mut d).is_some());
        assert_eq!(d.contours[0].bbox.min.x, 10.0);
    }

    #[test]
    fn new_change_drops_redo_and_delete_remaps_ops() {
        let mut d = doc_with_square();
        let sq2 = d.contours[0].contour.clone();
        d.add_contours(&[(sq2, String::new())], Vec2::new(20.0, 0.0));
        d.ops.push(Operation { contours: vec![0, 1], ..test_op() });
        let mut h = History::new(&d, "Start");

        d.selected = vec![true, false];
        assert_eq!(d.delete_selected(), 1);
        assert_eq!(d.ops[0].contours, vec![0], "operacja wskazuje teraz na przesuniętą krzywą");
        h.track(&mut d, true);
        h.undo(&mut d);
        assert_eq!(d.contours.len(), 2);
        assert_eq!(d.ops[0].contours, vec![0, 1]);

        d.stock.thickness = 12.0;
        h.track(&mut d, true);
        assert!(!h.can_redo());
        let last = h.entries().last().unwrap();
        assert_eq!((last.1, last.2.icon), ("Półfabrykat", Icon::Stock));
    }

    #[test]
    fn duplicated_and_disabled_ops_are_named() {
        let mut d = doc_with_square();
        d.ops.push(test_op());
        let mut h = History::new(&d, "Start");
        let copy = d.ops[0].clone();
        d.add_op(copy); // identyczna kopia
        h.track(&mut d, true);
        let last = h.entries().last().unwrap();
        assert_eq!((last.1, last.2.badge, last.2.op), ("Nowa operacja: Profil 1", Badge::Add, Some(1)));
        d.ops[1].enabled = false;
        h.track(&mut d, true);
        let last = h.entries().last().unwrap();
        assert_eq!((last.1, last.2.badge), ("Profil 1: wyłączona", Badge::Off));
    }

    fn test_op() -> Operation {
        Operation {
            name: "Profil 1".into(),
            enabled: true,
            kind: cam_ops::OpKind::Profile(Default::default()),
            tool_id: 1,
            cut: Default::default(),
            contours: vec![],
        }
    }
}
