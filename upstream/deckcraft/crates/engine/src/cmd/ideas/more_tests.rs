//! Tests of the ideas beyond the first fourteen: content-aware text ideas, several pictures,
//! title slides, colour variants, and taking an idea's shapes away again.
#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::sync::Arc;

use deckcraft_model::style::PictureFill;
use deckcraft_model::text::{Bullet, TextBody};
use deckcraft_model::{ColorRef, Geom, PhType, Placeholder};

use super::*;

fn title(text: &str) -> Shape {
    Shape {
        id: ShapeId(500),
        ph: Some(Placeholder { kind: PhType::Title, ..Default::default() }),
        xfrm: Some(Xfrm::new(50.0, 30.0, 860.0, 90.0)),
        text: Some(TextBody::from_text(text)),
        ..Default::default()
    }
}

fn body(id: u32, text: &str) -> Shape {
    Shape {
        id: ShapeId(id),
        text_box: true,
        xfrm: Some(Xfrm::new(50.0, 140.0, 860.0, 350.0)),
        text: Some(TextBody::from_text(text)),
        ..Default::default()
    }
}

fn picture(id: u32, w: f64, h: f64) -> Shape {
    Shape { id: ShapeId(id), kind: ShapeKind::Picture { fill: PictureFill::default() }, xfrm: Some(Xfrm::new(0.0, 0.0, w, h)), ..Default::default() }
}

fn session(shapes: Vec<Shape>) -> Session {
    let mut s = Session::with_new();
    let st = s.doc_mut().unwrap();
    let doc = Arc::make_mut(&mut st.doc);
    Arc::make_mut(&mut doc.slides[0]).shapes = shapes;
    s
}

fn ids(s: &Session) -> Vec<&'static str> {
    ideas_for(&s.doc().unwrap().doc, 0).iter().map(|i| i.0).collect()
}

fn shapes(s: &Session) -> Vec<Shape> {
    s.doc().unwrap().doc.slides[0].shapes.clone()
}

fn find(s: &Session, id: u32) -> Shape {
    shapes(s).into_iter().find(|x| x.id == ShapeId(id)).unwrap()
}

/// Every shape on the slide has a box inside it.
fn assert_on_slide(s: &Session, what: &str) {
    for sh in shapes(s) {
        let x = sh.xfrm.unwrap();
        assert!(x.x >= -0.01 && x.y >= -0.01 && x.x + x.w <= 960.01 && x.y + x.h <= 540.01 && x.w > 0.0 && x.h > 0.0, "{what}: {} {x:?}", sh.name);
    }
}

/// Every idea for the slide, in every colour variant, applies and keeps shapes on the slide;
/// then the slide goes back to how it was.
fn apply_all(shapes_in: Vec<Shape>) -> Vec<&'static str> {
    let probe = session(shapes_in.clone());
    let found = ids(&probe);
    for id in &found {
        for v in 0..variants_of(id).len() {
            let mut s = session(shapes_in.clone());
            s.execute("design.applyIdea", &json!({"id": id, "variant": v})).unwrap();
            assert_on_slide(&s, id);
            // The idea's ideas are the slide's own ideas still, and picking one again works.
            assert_eq!(ids(&s), found, "{id}");
            s.execute("design.applyIdea", &json!({"id": id, "variant": v})).unwrap();
            assert_on_slide(&s, id);
        }
    }
    found
}

#[test]
fn the_catalogue_has_forty_distinct_ideas() {
    let all: Vec<Idea> = catalogue().collect();
    let unique: HashSet<&str> = all.iter().map(|i| i.0).collect();
    assert_eq!(unique.len(), all.len());
    assert!(all.len() >= 40, "{}", all.len());
    // The ideas of earlier versions keep their ids.
    for id in [
        "fullBleed",
        "pictureLeft",
        "pictureRight",
        "framed",
        "pictureTop",
        "pictureBottom",
        "titleColumn",
        "centred",
        "titleBottom",
        "titleRight",
        "narrow",
        "titleMiddle",
        "titleLowerThird",
        "titleTopLeft",
    ] {
        assert!(unique.contains(id), "{id}");
        assert_eq!(variants_of(id).len(), 1, "{id} adds no shapes");
    }
    assert_eq!(variants_of("cards"), [SchemeSlot::Accent1, SchemeSlot::Accent2, SchemeSlot::Accent3]);
}

#[test]
fn text_ideas_follow_what_the_text_says() {
    let has = |text: &str, id: &str| ids(&session(vec![title("T"), body(501, text)])).contains(&id);
    assert!(has("Revenue grew 45% this year", "bigNumber"));
    assert!(!has("Revenue grew this year", "bigNumber"));
    assert!(has("“Simplicity is the ultimate sophistication.”\n— Leonardo", "quote"));
    assert!(has("We ship every week.", "statement"));
    assert!(has("Plan\nBuild\nTest\nShip", "cards"));
    assert!(has("Plan\nBuild\nTest\nShip", "agendaList"));
    assert!(has("Plan\nBuild\nTest\nShip", "chevrons"), "short points read as a process");
    assert!(!has("Plan\nBuild\nTest\nShip", "timeline"), "not dated, not numbered");
    assert!(has("2019 Founded\n2021 First customer\n2024 Series A", "timeline"));
    assert!(has("Step 1 Plan\nStep 2 Build\nStep 3 Ship", "timeline"));
    assert!(!has("one\ntwo", "cards"), "two points are not enough for cards");
    // Two text shapes compare side by side.
    let s = session(vec![title("T"), body(501, "Pros"), body(502, "Cons")]);
    assert!(ids(&s).contains(&"comparison"));
    // So does one list of two headed groups.
    let mut b = body(501, "Pros\nfast\ncheap\nCons\nloud");
    for i in [1, 2, 4] {
        b.text.as_mut().unwrap().paragraphs[i].level = 1;
    }
    assert!(ids(&session(vec![title("T"), b])).contains(&"comparison"));
}

#[test]
fn every_idea_applies_in_every_variant_and_stays_on_the_slide() {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut groups = vec![
        vec![title("Harbour"), body(501, "one\ntwo"), picture(502, 900.0, 450.0)],
        vec![title("Harbour"), picture(502, 900.0, 450.0)],
        vec![title("Harbour"), body(501, "one"), picture(502, 300.0, 200.0), picture(503, 200.0, 300.0), picture(504, 100.0, 100.0)],
        vec![title("Harbour"), picture(502, 300.0, 200.0), picture(503, 200.0, 300.0)],
        vec![title("Harbour"), picture(502, 300.0, 200.0), picture(503, 200.0, 300.0), picture(504, 100.0, 100.0)],
        vec![title("Harbour"), picture(502, 300.0, 200.0), picture(503, 200.0, 300.0), picture(504, 100.0, 100.0), picture(505, 120.0, 100.0)],
        vec![title("Results"), body(501, "Revenue grew 45% this year")],
        vec![title("Said"), body(501, "“Less is more.”\n— Mies")],
        vec![title("Plan"), body(501, "Plan\nBuild\nTest\nShip")],
        vec![title("Plan"), body(501, "Plan\nBuild\nTest\nShip\nLearn\nRepeat")],
        vec![title("History"), body(501, "2019 Founded\n2021 First customer\n2024 Series A\n2025 Europe\n2026 Asia")],
        vec![title("Choice"), body(501, "Pros"), body(502, "Cons")],
        vec![title("Closing")],
    ];
    let mut sub = body(501, "A subtitle");
    sub.ph = Some(Placeholder { kind: PhType::SubTitle, ..Default::default() });
    let mut ctr = title("Opening");
    ctr.ph = Some(Placeholder { kind: PhType::CtrTitle, ..Default::default() });
    groups.push(vec![ctr, sub]);
    for g in groups {
        seen.extend(apply_all(g));
    }
    let missing: Vec<&str> = catalogue().map(|i| i.0).filter(|id| !seen.contains(id)).collect();
    assert!(missing.is_empty(), "never offered: {missing:?}");
}

#[test]
fn a_title_slide_gets_title_ideas_before_the_text_ideas() {
    let mut sub = body(501, "A subtitle");
    sub.ph = Some(Placeholder { kind: PhType::SubTitle, ..Default::default() });
    let s = session(vec![title("Opening"), sub]);
    let found = ids(&s);
    assert_eq!(found[..TITLE_IDEAS.len()], TITLE_IDEAS.map(|i| i.0));
    assert!(found.contains(&"titleColumn"));
    let mut s = s;
    s.execute("design.applyIdea", &json!({"id": "titleMiddle"})).unwrap();
    let (t, sub) = (find(&s, 500).xfrm.unwrap(), find(&s, 501).xfrm.unwrap());
    assert!(sub.y >= t.y + t.h - 0.01, "the subtitle sits under the title: {t:?} {sub:?}");
}

#[test]
fn cards_split_the_text_and_the_next_idea_puts_it_back_with_edits() {
    let mut s = session(vec![title("Plan"), body(501, "Plan\nBuild\nTest\nShip")]);
    s.execute("edit.select", &json!({"ids": [501]})).ok();
    s.execute("design.applyIdea", &json!({"id": "cards", "variant": 1})).unwrap();
    let src = find(&s, 501);
    assert!(src.hidden, "the source hides behind its cards");
    assert!(!s.doc().unwrap().selection.shapes.contains(&ShapeId(501)), "a hidden shape leaves the selection");
    let cards: Vec<Shape> = shapes(&s).into_iter().filter(|x| decor::is_decor(x) && x.text.is_some()).collect();
    assert_eq!(cards.iter().map(|c| c.text.as_ref().unwrap().text()).collect::<Vec<_>>(), ["Plan", "Build", "Test", "Ship"]);
    // The card fills are the second accent.
    let accent2 = shapes(&s).into_iter().filter(|x| x.fill == Some(Fill::solid(ColorRef::scheme(SchemeSlot::Accent2)))).count();
    assert_eq!(accent2, 4, "one accent bar per card");
    // Edit a card, then pick another idea: the edit lands in the source, the cards go.
    let card = cards[1].id;
    let doc = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
    let slide = Arc::make_mut(&mut doc.slides[0]);
    slide.shapes.iter_mut().find(|x| x.id == card).unwrap().text = Some(TextBody::from_text("Build it well"));
    s.execute("design.applyIdea", &json!({"id": "titleColumn"})).unwrap();
    let all = shapes(&s);
    assert_eq!(all.len(), 2, "only the title and the text are left");
    let src = find(&s, 501);
    assert!(!src.hidden && src.tags.is_empty());
    assert_eq!(src.text.unwrap().text(), "Plan\nBuild it well\nTest\nShip");
    // One undo step each.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(find(&s, 501).hidden);
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(!find(&s, 501).hidden);
    assert_eq!(shapes(&s).len(), 2);
}

#[test]
fn numbered_steps_become_chevrons_that_keep_their_numbering() {
    let mut b = body(501, "Plan\nBuild\nShip");
    for p in &mut b.text.as_mut().unwrap().paragraphs {
        p.props.bullet = Some(Bullet::AutoNum { scheme: "arabicPeriod".into(), start_at: 1 });
    }
    let mut s = session(vec![title("How"), b]);
    assert!(ids(&s).contains(&"timeline"));
    s.execute("design.applyIdea", &json!({"id": "chevrons"})).unwrap();
    let arrows: Vec<Shape> = shapes(&s).into_iter().filter(|x| x.style.is_some() && decor::is_decor(x)).collect();
    assert_eq!(arrows.len(), 3);
    assert_eq!(arrows[0].geom, Geom::preset("homePlate"));
    assert_eq!(arrows[2].geom, Geom::preset("chevron"));
    assert!(arrows.windows(2).all(|w| w[0].xfrm.unwrap().x < w[1].xfrm.unwrap().x));
}

#[test]
fn ideas_with_shapes_survive_a_pptx_round_trip_and_are_taken_away_after_it() {
    let mut s = session(vec![title("Plan"), body(501, "Plan\nBuild\nTest\nShip")]);
    s.execute("design.applyIdea", &json!({"id": "agendaList"})).unwrap();
    let before = shapes(&s).len();
    let bytes = deckcraft_pptx::export(&s.doc().unwrap().doc).unwrap();
    let back = deckcraft_pptx::import(&bytes).unwrap();
    s.doc_mut().unwrap().doc = Arc::new(back);
    assert_eq!(shapes(&s).len(), before);
    assert!(shapes(&s).iter().any(decor::is_decor), "the tags survive the save");
    assert!(ids(&s).contains(&"cards"));
    s.execute("design.applyIdea", &json!({"id": "centred"})).unwrap();
    let all = shapes(&s);
    assert_eq!(all.len(), 2, "{:?}", all.iter().map(|x| &x.name).collect::<Vec<_>>());
    assert!(all.iter().all(|x| !x.hidden));
    assert_eq!(all[1].text.as_ref().unwrap().text(), "Plan\nBuild\nTest\nShip");
}

#[test]
fn a_circle_crop_is_undone_by_the_next_idea() {
    let mut s = session(vec![title("Harbour"), body(501, "one"), picture(502, 900.0, 450.0)]);
    s.execute("design.applyIdea", &json!({"id": "pictureCircle"})).unwrap();
    let pic = find(&s, 502);
    assert_eq!(pic.geom, Geom::preset("ellipse"));
    let x = pic.xfrm.unwrap();
    assert!((x.w - x.h).abs() < 1e-9, "a circle, not an oval");
    s.execute("design.applyIdea", &json!({"id": "pictureLeft"})).unwrap();
    assert_eq!(find(&s, 502).geom, Geom::default());
    assert_eq!(shapes(&s).len(), 3, "the ring is gone");
}

#[test]
fn several_pictures_are_laid_out_without_overlapping() {
    let pics = vec![title("Harbour"), picture(502, 300.0, 200.0), picture(503, 200.0, 300.0), picture(504, 100.0, 100.0), picture(505, 120.0, 100.0)];
    for id in ["grid", "collage", "filmstrip", "mosaic"] {
        let mut s = session(pics.clone());
        s.execute("design.applyIdea", &json!({"id": id})).unwrap();
        let boxes: Vec<Xfrm> = (502..=505).map(|i| find(&s, i).xfrm.unwrap()).collect();
        for (a, ba) in boxes.iter().enumerate() {
            for bb in &boxes[a + 1..] {
                let apart = ba.x + ba.w <= bb.x + 0.01 || bb.x + bb.w <= ba.x + 0.01 || ba.y + ba.h <= bb.y + 0.01 || bb.y + bb.h <= ba.y + 0.01;
                assert!(apart, "{id}: {ba:?} {bb:?}");
            }
        }
    }
    // Five pictures are more than the ideas lay out.
    let mut five = pics;
    five.push(picture(506, 100.0, 100.0));
    assert!(ids(&session(five)).is_empty());
}

#[test]
fn variants_are_listed_and_checked() {
    let mut s = session(vec![title("Plan"), body(501, "Plan\nBuild\nTest\nShip")]);
    let list = s.execute("design.ideas", &json!({})).unwrap();
    let cards = list.as_array().unwrap().iter().find(|i| i["id"] == "cards").unwrap();
    assert_eq!(cards["variants"], 3);
    let centred = list.as_array().unwrap().iter().find(|i| i["id"] == "centred").unwrap();
    assert_eq!(centred["variants"], 1);
    assert!(s.execute("design.applyIdea", &json!({"id": "cards", "variant": 3})).is_err());
    assert!(s.execute("design.applyIdea", &json!({"id": "centred", "variant": 1})).is_err());
    assert!(s.execute("design.applyIdea", &json!({"id": "cards", "variant": -1})).is_err());
    s.execute("design.applyIdea", &json!({"id": "cards", "variant": 2})).unwrap();
    assert!(shapes(&s).iter().any(|x| x.fill == Some(Fill::solid(ColorRef::scheme(SchemeSlot::Accent3)))));
}

#[test]
fn a_big_number_shows_the_figure_in_the_accent_colour() {
    let mut s = session(vec![title("Results"), body(501, "Revenue grew 45% this year")]);
    s.execute("design.applyIdea", &json!({"id": "bigNumber", "variant": 1})).unwrap();
    let label = shapes(&s).into_iter().find(|x| x.text.as_ref().is_some_and(|t| t.text() == "45%")).unwrap();
    assert!(decor::is_decor(&label));
    let run = &label.text.unwrap().paragraphs[0].runs[0];
    assert_eq!(run.props.fill, Some(Fill::solid(ColorRef::scheme(SchemeSlot::Accent2))));
    assert!(!find(&s, 501).hidden, "the text itself stays");
}
