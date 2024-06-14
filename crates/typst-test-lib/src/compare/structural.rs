use typst::layout::{Frame, FrameItem, Page};
use typst::model::Document;

use super::{Error, PageError};

pub fn compare_documents(
    output: &Document,
    reference: &Document,
    fail_fast: bool,
) -> Result<(), Error> {
    if output.date == reference.date
        && output.title == reference.title
        && output.author == reference.author
        && output.keywords == reference.keywords
        && output.pages.len() == reference.pages.len()
    {
        todo!()
    }

    if output.pages.len() != reference.pages.len() {
        return Err(Error::PageCount {
            output: output.pages.len(),
            reference: reference.pages.len(),
        });
    }

    let mut page_errors = if fail_fast {
        vec![]
    } else {
        Vec::with_capacity(output.pages.len())
    };

    for (idx, (a, b)) in Iterator::zip(output.pages.iter(), reference.pages.iter()).enumerate() {
        if let Err(err) = compare_page(&a, &b) {
            page_errors.push((idx, err));

            if fail_fast {
                break;
            }
        }
    }

    if page_errors.len() != 0 {
        page_errors.shrink_to_fit();
        return Err(Error::Page { pages: page_errors });
    }

    Ok(())
}

fn compare_page(a: &Page, b: &Page) -> Result<(), PageError> {
    if a.number == b.number && a.numbering == b.numbering && frame_eq(&a.frame, &b.frame) {
        Ok(())
    } else {
        Err(PageError::Structure)
    }
}

fn frame_eq(a: &Frame, b: &Frame) -> bool {
    a.kind() == b.kind()
        && a.size() == b.size()
        && a.baseline() == b.baseline()
        && Iterator::zip(a.items(), b.items()).all(|(a, b)| a.0 == b.0 && frame_item_eq(&a.1, &b.1))
}

// TODO: implement actual comparison, if it is feasible
fn frame_item_eq(a: &FrameItem, b: &FrameItem) -> bool {
    match (a, b) {
        (FrameItem::Group(_), FrameItem::Group(_)) => true,
        (FrameItem::Text(_), FrameItem::Text(_)) => true,
        (FrameItem::Shape(_, _), FrameItem::Shape(_, _)) => true,
        (FrameItem::Image(_, _, _), FrameItem::Image(_, _, _)) => true,
        (FrameItem::Meta(_, _), FrameItem::Meta(_, _)) => true,
        _ => false,
    }
}

// TODO: tests
