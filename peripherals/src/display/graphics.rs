use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{DrawTarget, IntoStorage, Point, Size};
use embedded_graphics::{
    pixelcolor::raw::{RawData, RawU16},
    primitives::Rectangle,
};
use embedded_graphics::{prelude::OriginDimensions, Pixel};

use embedded_hal::digital::v2::OutputPin;

use display_interface::WriteOnlyDataCommand;

use super::{Error, Orientation, ST7789};

impl<DI, RST, BL, PinE> ST7789<DI, RST, BL>
where
    DI: WriteOnlyDataCommand,
    RST: OutputPin<Error = PinE>,
    BL: OutputPin<Error = PinE>,
{
    /// Returns the bounding box for the entire framebuffer.
    fn framebuffer_bounding_box(&self) -> Rectangle {
        let size = match self.orientation {
            Orientation::Portrait | Orientation::PortraitSwapped => Size::new(240, 320),
            Orientation::Landscape | Orientation::LandscapeSwapped => Size::new(320, 240),
        };

        Rectangle::new(Point::zero(), size)
    }
}

impl<DI, RST, BL, PinE> DrawTarget for ST7789<DI, RST, BL>
where
    DI: WriteOnlyDataCommand,
    RST: OutputPin<Error = PinE>,
    BL: OutputPin<Error = PinE>,
{
    type Error = Error<PinE>;
    type Color = Rgb565;

    #[cfg(not(feature = "batch"))]
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let color = RawU16::from(pixel.1).into_inner();
            let x = pixel.0.x as u16;
            let y = pixel.0.y as u16;

            self.set_pixel(x, y, color)?;
        }

        Ok(())
    }

    #[cfg(feature = "batch")]
    fn draw_iter<T>(&mut self, item: T) -> Result<(), Self::Error>
    where
        T: IntoIterator<Item = Pixel<Rgb565>>,
    {
        use crate::batch::DrawBatch;

        self.draw_batch(item)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        if let Some(bottom_right) = area.bottom_right() {
            let mut count = 0u32;
            let max = area.size.width * area.size.height;

            let mut colors = colors
                .into_iter()
                .take_while(|_| {
                    count += 1;
                    count <= max
                })
                .map(|color| RawU16::from(color).into_inner());

            let sx = area.top_left.x as u16;
            let sy = area.top_left.y as u16;
            let ex = bottom_right.x as u16;
            let ey = bottom_right.y as u16;
            self.set_pixels(sx, sy, ex, ey, &mut colors)
        } else {
            // nothing to draw
            Ok(())
        }
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.intersection(&self.framebuffer_bounding_box());

        if let Some(bottom_right) = area.bottom_right() {
            let mut count = 0u32;
            let max = area.size.width * area.size.height;

            let mut colors = core::iter::repeat(color.into_storage()).take_while(|_| {
                count += 1;
                count <= max
            });

            let sx = area.top_left.x as u16;
            let sy = area.top_left.y as u16;
            let ex = bottom_right.x as u16;
            let ey = bottom_right.y as u16;
            self.set_pixels(sx, sy, ex, ey, &mut colors)
        } else {
            // nothing to draw
            Ok(())
        }
    }

    fn clear(&mut self, color: Rgb565) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        let color16 = RawU16::from(color).into_inner();
        let colors = (0..(240*240)).map(|_| color16); // blank entire HW RAM contents

        self.set_pixels(0, 0, 239, 239, colors)
    }
}

impl<DI, RST, BL, PinE> OriginDimensions for ST7789<DI, RST, BL>
where
    DI: WriteOnlyDataCommand,
    RST: OutputPin<Error = PinE>,
    BL: OutputPin<Error = PinE>,
{
    fn size(&self) -> Size {
        Size::new(240, 240) // visible area, not RAM-pixel size
    }
}