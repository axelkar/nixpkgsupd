use itertools::Itertools;

pub fn get_two_pointers<'de>(
    input: impl sonic_rs::JsonInput<'de>,
    p1: impl IntoIterator<Item = impl sonic_rs::Index>,
    p2: impl IntoIterator<Item = impl sonic_rs::Index>,
) -> Result<(sonic_rs::LazyValue<'de>, sonic_rs::LazyValue<'de>), sonic_rs::Error> {
    let mut tree = sonic_rs::PointerTree::new();

    tree.add_path(p1);
    tree.add_path(p2);

    Ok(sonic_rs::get_many(input, &tree)?
        .into_iter()
        .collect_tuple()
        .unwrap())
}

pub fn get_three_pointers<'de>(
    input: impl sonic_rs::JsonInput<'de>,
    p1: impl IntoIterator<Item = impl sonic_rs::Index>,
    p2: impl IntoIterator<Item = impl sonic_rs::Index>,
    p3: impl IntoIterator<Item = impl sonic_rs::Index>,
) -> Result<
    (
        sonic_rs::LazyValue<'de>,
        sonic_rs::LazyValue<'de>,
        sonic_rs::LazyValue<'de>,
    ),
    sonic_rs::Error,
> {
    let mut tree = sonic_rs::PointerTree::new();

    tree.add_path(p1);
    tree.add_path(p2);
    tree.add_path(p3);

    Ok(sonic_rs::get_many(input, &tree)?
        .into_iter()
        .collect_tuple()
        .unwrap())
}

pub fn get_opt_json<'de>(
    input: impl sonic_rs::JsonInput<'de>,
    p1: impl IntoIterator<Item = impl sonic_rs::Index>,
) -> Result<Option<sonic_rs::LazyValue<'de>>, sonic_rs::Error> {
    match sonic_rs::get(input, p1) {
        Err(err) if err.is_not_found() => Ok(None),
        res => res.map(Some),
    }
}
