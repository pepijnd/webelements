use std::str::FromStr;

use elem::ElemTy;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{InputEvent};

use crate::{Error, Result};

pub use web_sys::MouseEvent;

pub mod elem {
    use wasm_bindgen::JsCast;
    use we_derive::element_types;
    pub trait ElemTy {
        type Elem: AsRef<web_sys::Element>;
        fn make() -> crate::Result<Self::Elem>;
    }
    element_types!();
}

pub trait WebElementBuilder
{
    type Elem: ElemTy;
    fn build() -> Result<Self>
    where
        Self: std::marker::Sized;
}

pub trait WebElement: WebElementBuilder
{
    fn init(&mut self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Element<E>
where
    E: ElemTy,
{
    element: E::Elem,
}

impl<E> AsRef<Element<E>> for Element<E>
where
    E: ElemTy,
{
    fn as_ref(&self) -> &Element<E> {
        self.root()
    }
}

impl<E> Element<E>
where
    E: ElemTy,
{
    pub fn new() -> Result<Element<E>> {
        let element = E::make()?;
        Ok(Self { element })
    }

    pub fn from_element(element: E::Elem) -> Self {
        Self {
            element
        }
    }

    fn as_element(&self) -> &web_sys::Element {
        &self.element.as_ref()
    }

    fn as_node(&self) -> &web_sys::Node {
        &self.element.as_ref()
    }

    pub fn append<T: ElemTy>(&self, other: impl AsRef<Element<T>>) -> Result<()> {
        self.element
            .as_ref()
            .append_child(other.as_ref().as_node())?;
        Ok(())
    }

    pub fn append_list<T: ElemTy>(
        &self,
        items: impl IntoIterator<Item = impl AsRef<Element<T>>>,
    ) -> Result<()> {
        items.into_iter().try_for_each(|i| self.append(i))
    }

    pub fn root(&self) -> &Element<E> {
        &self
    }

    pub fn has_class(&self, class: impl AsRef<str>) -> bool {
        let class_string: String = self.as_element().class_name();
        for class_name in class_string.split_whitespace() {
            if class.as_ref() == class_name {
                return true;
            }
        }
        false
    }

    pub fn toggle_class(&self, class: impl AsRef<str>) {
        for class in class.as_ref().split_whitespace() {
            if self.has_class(class) {
                self.remove_class(class);
            } else {
                self.add_class(class);
            }
        }
    }

    pub fn add_class(&self, class: impl AsRef<str>) {
        for class in class.as_ref().split_whitespace() {
            if !self.has_class(class) {
                let mut class_string: String = self.as_element().class_name();
                class_string.push_str(&format!(" {}", class));
                self.as_element().set_class_name(class_string.trim());
            }
        }
    }

    pub fn set_class(&self, class: impl AsRef<str>) {
        self.as_element().set_class_name(class.as_ref());
    }

    pub fn clear_class(&self) {
        self.as_element().set_class_name("");
    }

    pub fn remove_class(&self, class: impl AsRef<str>) {
        for class in class.as_ref().split_whitespace() {
            if self.has_class(class) {
                let class_string = self.as_element().class_name();
                let mut new_string = Vec::<&str>::new();
                for class_name in class_string.split_whitespace() {
                    if class_name != class {
                        new_string.push(class_name)
                    }
                }
                let new_string = new_string.join(" ");
                self.as_element().set_class_name(new_string.trim());
            }
        }
    }

    pub fn set_text(&self, text: impl AsRef<str>) {
        self.as_element().set_inner_html(text.as_ref())
    }

    pub fn set_attr(&self, name: impl AsRef<str>, value: impl AsRef<str>) -> Result<()> {
        self.as_element()
            .set_attribute(name.as_ref(), value.as_ref())?;
        Ok(())
    }

    pub fn del_attr(&self, name: impl AsRef<str>) -> Result<()> {
        self.as_element()
            .remove_attribute(name.as_ref())?;
        Ok(())
    }

    pub fn attr(&self, name: impl AsRef<str>) -> Option<String> {
        self.as_element()
            .get_attribute(name.as_ref())
    }

    pub fn on_click(&self, callback: impl FnMut(MouseEvent) + 'static ) -> Result<()> {
        let closure = Closure::wrap(Box::new(callback) as Box<dyn FnMut(MouseEvent)>);
        self.as_element()
            .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
            .map_err(Error::JsError)?;
        closure.forget();
        Ok(())
    }
}

impl Element<elem::Base> {
    pub fn style(&self) -> web_sys::CssStyleDeclaration {
        self.element.style()
    }
}

impl Element<elem::Button> {
}

impl Element<elem::Input> {
    pub fn on_input(&self, callback: impl FnMut(InputEvent) + 'static ) -> Result<()> {
        let closure = Closure::wrap(Box::new(callback) as Box<dyn FnMut(InputEvent)>);
        self.as_element()
            .add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())
            .map_err(Error::JsError)?;
        closure.forget();
        Ok(())
    }

    pub fn set_min<T: ToString>(&self, value: T) {
        self.element.set_min(&value.to_string())
    }

    pub fn set_max<T: ToString>(&self, value: T) {
        self.element.set_max(&value.to_string())
    }

    pub fn set_value<T: ToString>(&self, value: T) {
        self.element.set_value(&value.to_string())
    }

    pub fn get_value<T: FromStr>(&self) -> Result<T> {
        self.element.value().parse::<T>().map_err(|_| Error::Value)
    }
}