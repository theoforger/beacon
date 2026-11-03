// SPDX-License-Identifier: GPL-3.0-or-later

//! A texture that stays sharp when drawn much smaller than its pixels.
//! `GtkPicture` and `GtkImage` draw a texture with a linear filter, which
//! turns soft and jagged when a cover decoded for the largest grid tile is
//! drawn in a narrower one. This paintable draws it with GSK's trilinear
//! (mipmapped) filter instead, so a cover is decoded once and looks right at
//! any size the grid gives it.

use relm4::gtk::{self, gdk, glib, graphene, gsk, prelude::*, subclass::prelude::*};

mod imp {
    use std::cell::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct SmoothTexture {
        pub texture: OnceCell<gdk::Texture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SmoothTexture {
        const NAME: &'static str = "BeaconSmoothTexture";
        type Type = super::SmoothTexture;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for SmoothTexture {}

    impl gdk::subclass::paintable::PaintableImpl for SmoothTexture {
        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::STATIC_SIZE | gdk::PaintableFlags::STATIC_CONTENTS
        }

        fn intrinsic_width(&self) -> i32 {
            self.texture.get().map_or(0, |t| t.width())
        }

        fn intrinsic_height(&self) -> i32 {
            self.texture.get().map_or(0, |t| t.height())
        }

        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let (Some(texture), Some(snapshot)) =
                (self.texture.get(), snapshot.downcast_ref::<gtk::Snapshot>())
            else {
                return;
            };
            snapshot.append_scaled_texture(
                texture,
                gsk::ScalingFilter::Trilinear,
                &graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
            );
        }
    }
}

glib::wrapper! {
    pub struct SmoothTexture(ObjectSubclass<imp::SmoothTexture>)
        @implements gdk::Paintable;
}

impl SmoothTexture {
    pub fn new(texture: gdk::Texture) -> Self {
        let this: Self = glib::Object::new();
        let _ = this.imp().texture.set(texture);
        this
    }
}
