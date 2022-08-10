use std::borrow::Cow;

use reqwest::multipart::Form;

#[derive(Default)]
pub struct FormBuilder {
    form: Form,
}

impl FormBuilder {
    pub fn add(&mut self, name: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) {
        let form = std::mem::take(&mut self.form);
        self.form = form.text(name.into(), value.into());
    }
}

pub struct EditBuilder {
    form: FormBuilder,
}

impl EditBuilder {
    pub fn for_page_id(id: u32) -> EditBuilder {
        let mut form = FormBuilder::default();
        form.add("pageid", format!("{id}"));
        EditBuilder { form }
    }

    pub fn for_title(title: impl Into<Cow<'static, str>>) -> EditBuilder {
        let mut form = FormBuilder::default();
        form.add("title", title);
        EditBuilder { form }
    }
}

impl EditBuilder {}
