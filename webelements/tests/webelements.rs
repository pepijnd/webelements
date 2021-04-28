use webelements::{we_builder, WebElement, WebElementBuilder};

use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);


#[we_builder(
    <div class="my-element" attr="value">
        <div class="repeated" we_field="repeated" we_repeat=5 />
    </div>
)]
#[derive(Debug, Clone, WebElement)]
struct MyElement {}

#[we_builder(
    <div class="my-element" attr="value">
        <MyElement we_field="elem" we_repeat=2 we_element />
    </div>
)]
#[derive(Debug, Clone, WebElement)]
struct OtherElement {}


#[wasm_bindgen_test]
fn test_we_elements() {
    let el = OtherElement::build().unwrap();
    assert_eq!(el.elem.first().unwrap().repeated.len(), 5)
}