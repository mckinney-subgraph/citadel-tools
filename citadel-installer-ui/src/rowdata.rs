
use gio::prelude::*;
use std::fmt;

pub mod row_data {

    use super::*;

    use glib::subclass;
    use glib::subclass::prelude::*;
    use glib::translate::*;

    mod imp {
        use super::*;
        use std::cell::RefCell;

        pub struct RowData {
            model: RefCell<Option<String>>,
            path: RefCell<Option<String>>,
            size: RefCell<Option<String>>,
            removable: RefCell<bool>,
        }

        static PROPERTIES: [subclass::Property; 4] = [
            subclass::Property("model", |name| {
                glib::ParamSpec::string(
                    name,
                    "Model",
                    "Model",
                    None, // Default value
                    glib::ParamFlags::READWRITE,
                )
            }),
            subclass::Property("path", |name| {
                glib::ParamSpec::string(
                    name,
                    "Path",
                    "Path",
                    None, // Default value
                    glib::ParamFlags::READWRITE,
                )
            }),
            subclass::Property("size", |name| {
                glib::ParamSpec::string(
                    name,
                    "Size",
                    "Size",
                    None, // Default value
                    glib::ParamFlags::READWRITE,
                )
            }),
            subclass::Property("removable", |name| {
                glib::ParamSpec::boolean(
                    name,
                    "Removable",
                    "Removable",
                    false, // Default value
                    glib::ParamFlags::READWRITE,
                )
            }),
        ];

        impl ObjectSubclass for RowData {
            const NAME: &'static str = "RowData";
            type ParentType = glib::Object;
            type Instance = subclass::simple::InstanceStruct<Self>;
            type Class = subclass::simple::ClassStruct<Self>;

            glib_object_subclass!();

            fn class_init(klass: &mut Self::Class) {
                klass.install_properties(&PROPERTIES);
            }

            fn new() -> Self {
                Self {
                    model: RefCell::new(None),
                    path: RefCell::new(None),
                    size: RefCell::new(None),
                    removable: RefCell::new(false),
                }
            }
        }

        impl ObjectImpl for RowData {
            glib_object_impl!();

            fn set_property(&self, _obj: &glib::Object, id: usize, value: &glib::Value) {
                let prop = &PROPERTIES[id];

                match *prop {
                    subclass::Property("model", ..) => {
                        let model = value
                            .get()
                            .expect("type conformity checked by `Object::set_property`");
                        self.model.replace(model);
                    }
                    subclass::Property("path", ..) => {
                        let path = value
                            .get()
                            .expect("type conformity checked by `Object::set_property`");
                        self.path.replace(path);
                    }
                    subclass::Property("size", ..) => {
                        let size = value
                            .get()
                            .expect("type conformity checked by `Object::set_property`");
                        self.size.replace(size);
                    }
                    subclass::Property("removable", ..) => {
                        let removable = value
                            .get_some()
                            .expect("type conformity checked by `Object::set_property`");
                        self.removable.replace(removable);
                    }
                    _ => unimplemented!(),
                }
            }

            fn get_property(&self, _obj: &glib::Object, id: usize) -> Result<glib::Value, ()> {
                let prop = &PROPERTIES[id];

                match *prop {
                    subclass::Property("model", ..) => Ok(self.model.borrow().to_value()),
                    subclass::Property("path", ..) => Ok(self.path.borrow().to_value()),
                    subclass::Property("size", ..) => Ok(self.size.borrow().to_value()),
                    subclass::Property("removable", ..) => Ok(self.removable.borrow().to_value()),
                    _ => unimplemented!(),
                }
            }
        }
    }

    glib_wrapper! {
        pub struct RowData(Object<subclass::simple::InstanceStruct<imp::RowData>, subclass::simple::ClassStruct<imp::RowData>, RowDataClass>);

        match fn {
            get_type => || imp::RowData::get_type().to_glib(),
        }
    }

    impl RowData {
        pub fn new(model: &str, path: &str, size: &str, removable: bool) -> RowData {
            glib::Object::new(Self::static_type(), &[("model", &model), ("path", &path), ("size", &size), ("removable", &removable)])
                .expect("Failed to create row data")
                .downcast()
                .expect("Created row data is of wrong type")
        }
    }

    impl fmt::Display for RowData {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{:?}", self.1)
        }
    }
}
