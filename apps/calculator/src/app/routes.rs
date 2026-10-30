use super::*;

pub(super) fn create() -> Router {
    let mut builder = Router::memory("/calculator/standard").dev_override(false);
    for mode in [Mode::Standard, Mode::Scientific]
        .into_iter()
        .chain(Category::ALL.into_iter().map(Mode::Converter))
    {
        builder = builder.route(mode.path(), move || {
            let state = creamui_reactive::use_context::<State>();
            let geometry = creamui_reactive::use_context::<Geometry>();
            match mode {
                Mode::Standard => state.calculator_view(false, geometry),
                Mode::Scientific => state.calculator_view(true, geometry),
                Mode::Converter(category) => state.converter_view(category, geometry),
            }
        });
    }
    builder.build().expect("calculator routes are valid")
}
