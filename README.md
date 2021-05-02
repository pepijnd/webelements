# webelements

webelements abstracts over the web-sys crate to give a more frienly api.
building elements becomes easy using the `WebElement` macros that automatically create rust code from html

# Example

```rust
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
wasm_bindgen_test_configure!(run_in_browser);

use webelements::{we_builder, WebElement, WebElementBuilder};

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
```

# Details

the we-derive crate contains the macros that transform the html.
the webelements crate contains the code that abstracts the web-sys code
