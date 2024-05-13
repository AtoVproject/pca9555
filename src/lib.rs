#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embedded_hal_async::i2c::{I2c, SevenBitAddress};
use seq_macro::seq;

#[derive(Debug)]
pub enum Error<I> {
    /// I2c bus error
    I2c(I),
    /// Connection error (device not found)
    Conn,
    /// Address error (invalid or out of bounds)
    Address,
    /// Port error (invalid or out of bounds)
    Port,
}

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reg {
    InputPort0 = 0x00,
    InputPort1 = 0x01,
    OutputPort0 = 0x02,
    OutputPort1 = 0x03,
    PolarityInversion0 = 0x04,
    PolarityInversion1 = 0x05,
    Configuration0 = 0x06,
    Configuration1 = 0x07,
}

#[derive(PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

impl From<Reg> for u8 {
    fn from(r: Reg) -> u8 {
        r as u8
    }
}

pub mod mode {
    pub trait HasOutput {}
    pub trait HasInput {}

    pub struct Input;
    impl HasInput for Input {}

    pub struct Output;
    impl HasOutput for Output {}
}

pub struct Pin<'a, MODE, I2C> {
    driver: &'a WrappedDriver<I2C>,
    mask: u32,
    _m: PhantomData<MODE>,
}

pub struct Address(u8, u8, u8);

impl From<Address> for u8 {
    fn from(a: Address) -> Self {
        0x20 | (a.2 << 2) | (a.1 << 1) | a.0
    }
}

impl<'a, MODE, I2C, I> Pin<'a, MODE, I2C>
where
    I2C: I2c<Error = I>,
{
    pub(crate) fn new(driver: &'a WrappedDriver<I2C>, pin_no: u8) -> Self {
        Self {
            driver,
            mask: 1 << pin_no,
            _m: PhantomData,
        }
    }

    pub fn mask(&self) -> u32 {
        self.mask
    }
}

impl<'a, MODE, I2C, I> Pin<'a, MODE, I2C>
where
    I2C: I2c<Error = I>,
{
    /// Configure this pin as an input.
    pub async fn into_input(self) -> Result<Pin<'a, mode::Input, I2C>, Error<I>> {
        let mut driver = self.driver.lock().await;
        driver
            .set_direction(self.mask, Direction::Input, false)
            .await?;
        Ok(Pin {
            mask: self.mask,
            driver: self.driver,
            _m: PhantomData,
        })
    }

    /// Configure this pin as an output with an initial LOW state.
    pub async fn into_output(self) -> Result<Pin<'a, mode::Output, I2C>, Error<I>> {
        let mut driver = self.driver.lock().await;
        driver
            .set_direction(self.mask, crate::Direction::Output, false)
            .await?;
        Ok(Pin {
            mask: self.mask,
            driver: self.driver,
            _m: PhantomData,
        })
    }

    /// Configure this pin as an output with an initial HIGH state.
    ///
    pub async fn into_output_high(self) -> Result<Pin<'a, mode::Output, I2C>, Error<I>> {
        let mut driver = self.driver.lock().await;
        driver
            .set_direction(self.mask, crate::Direction::Output, true)
            .await?;
        Ok(Pin {
            mask: self.mask,
            driver: self.driver,
            _m: PhantomData,
        })
    }

    /// Turn on hardware polarity inversion for this pin.
    pub async fn into_inverted(self) -> Result<Self, Error<I>> {
        let mut driver = self.driver.lock().await;
        driver.set_polarity(self.mask, true).await?;
        Ok(self)
    }

    /// Set hardware polarity inversion for this pin.
    pub async fn set_inverted(&mut self, inverted: bool) -> Result<(), Error<I>> {
        let mut driver = self.driver.lock().await;
        driver.set_polarity(self.mask, inverted).await?;
        Ok(())
    }
}

impl<'a, MODE: mode::HasInput, I2C, I> Pin<'a, MODE, I2C>
where
    I2C: I2c<Error = I>,
{
    /// Read the pin's input state and return `true` if it is HIGH.
    pub async fn is_high(&mut self) -> Result<bool, Error<I>> {
        let mut driver = self.driver.lock().await;
        Ok(driver.get(self.mask, 0).await? == self.mask)
    }

    /// Read the pin's input state and return `true` if it is LOW.
    pub async fn is_low(&mut self) -> Result<bool, Error<I>> {
        let mut driver = self.driver.lock().await;
        Ok(driver.get(0, self.mask).await? == self.mask)
    }
}

impl<'a, MODE: mode::HasOutput, I2C, I> Pin<'a, MODE, I2C>
where
    I2C: I2c<Error = I>,
{
    /// Set the pin's output state to HIGH.
    ///
    /// Note that this can have different electrical meanings depending on the port-expander chip.
    pub async fn set_high(&mut self) -> Result<(), Error<I>> {
        let mut driver = self.driver.lock().await;
        driver.set(self.mask, 0).await?;
        Ok(())
    }

    /// Set the pin's output state to LOW.
    ///
    /// Note that this can have different electrical meanings depending on the port-expander chip.
    pub async fn set_low(&mut self) -> Result<(), Error<I>> {
        let mut driver = self.driver.lock().await;
        driver.set(0, self.mask).await?;
        Ok(())
    }

    /// Return `true` if the pin's output state is HIGH.
    ///
    /// This method does **not** read the pin's electrical state.
    pub async fn is_set_high(&self) -> Result<bool, Error<I>> {
        let mut driver = self.driver.lock().await;
        Ok(driver.is_set(self.mask, 0) == self.mask)
    }

    /// Return `true` if the pin's output state is LOW.
    ///
    /// This method does **not** read the pin's electrical state.
    pub async fn is_set_low(&self) -> Result<bool, Error<I>> {
        let mut driver = self.driver.lock().await;
        Ok(driver.is_set(0, self.mask) == self.mask)
    }

    /// Toggle the pin's output state.
    pub async fn toggle(&mut self) -> Result<(), Error<I>> {
        let mut driver = self.driver.lock().await;
        driver.toggle(self.mask).await?;
        Ok(())
    }
}

seq!(N in 0..16 {
    pub struct Parts<'a, I2C, I>
    where
        I2C: I2c<Error = I> + 'a
    {
        #(
            pub pin~N: Pin<'a, mode::Input, I2C>,
        )*
    }
});

pub type WrappedDriver<I2C> = Mutex<CriticalSectionRawMutex, PortDriver<I2C>>;

pub struct Pca9555<I2C> {
    driver: WrappedDriver<I2C>,
}

seq!(N in 0..16 {
    impl<I2C, I> Pca9555<I2C>
    where
        I2C: I2c<Error = I>,
    {
        // TODO: Use address contsructor
        pub fn new(i2c: I2C, address: SevenBitAddress) -> Self {
            Self {
                driver: Mutex::new(PortDriver::new(i2c, address)),
            }
        }

        pub fn split(&mut self) -> Parts<'_, I2C, I> {
            Parts {
                #(
                    pin~N: Pin::new(&self.driver, 0),
                )*
            }
        }
    }
});

pub struct PortDriver<I2C> {
    address: SevenBitAddress,
    i2c: I2C,
    out: u16,
}

impl<I2C, I> PortDriver<I2C>
where
    I2C: I2c<Error = I>,
{
    fn new(i2c: I2C, address: SevenBitAddress) -> Self {
        // TODO: check if we need to do some initialization, otherwise rename this into new and
        // remove Result
        Self {
            address,
            i2c,
            out: 0xffff,
        }
    }

    async fn write_reg(&mut self, reg: Reg, value: u8) -> Result<(), Error<I>> {
        self.i2c
            .write(self.address, &[reg.into(), value])
            .await
            .map_err(Error::I2c)
    }

    async fn read_reg(&mut self, reg: Reg) -> Result<u8, Error<I>> {
        let mut buf = [0x00];
        self.i2c
            .write_read(self.address, &[reg.into()], &mut buf)
            .await
            .map_err(Error::I2c)?;
        Ok(buf[0])
    }

    async fn update_reg(&mut self, reg: Reg, mask_set: u8, mask_clear: u8) -> Result<(), Error<I>> {
        let reg = reg.into();
        let mut buf = [0x00];
        self.i2c
            .write_read(self.address, &[reg], &mut buf)
            .await
            .map_err(Error::I2c)?;
        buf[0] |= mask_set;
        buf[0] &= !mask_clear;
        self.i2c
            .write(self.address, &[reg, buf[0]])
            .await
            .map_err(Error::I2c)?;
        Ok(())
    }

    async fn set(&mut self, mask_high: u32, mask_low: u32) -> Result<(), Error<I>> {
        self.out |= mask_high as u16;
        self.out &= !mask_low as u16;
        if (mask_high | mask_low) & 0x00FF != 0 {
            self.write_reg(Reg::OutputPort0, (self.out & 0xFF) as u8)
                .await?;
        }
        if (mask_high | mask_low) & 0xFF00 != 0 {
            self.write_reg(Reg::OutputPort1, (self.out >> 8) as u8)
                .await?;
        }
        Ok(())
    }

    fn is_set(&mut self, mask_high: u32, mask_low: u32) -> u32 {
        ((self.out as u32) & mask_high) | (!(self.out as u32) & mask_low)
    }

    async fn toggle(&mut self, mask: u32) -> Result<(), Error<I>> {
        // for all pins which are currently low, make them high.
        let mask_high = self.is_set(0, mask);
        // for all pins which are currently high, make them low.
        let mask_low = self.is_set(mask, 0);
        self.set(mask_high, mask_low).await
    }

    async fn get(&mut self, mask_high: u32, mask_low: u32) -> Result<u32, Error<I>> {
        let io0 = if (mask_high | mask_low) & 0x00FF != 0 {
            self.read_reg(Reg::InputPort0).await?
        } else {
            0
        };
        let io1 = if (mask_high | mask_low) & 0xFF00 != 0 {
            self.read_reg(Reg::InputPort1).await?
        } else {
            0
        };
        let in_ = ((io1 as u32) << 8) | io0 as u32;
        Ok((in_ & mask_high) | (!in_ & mask_low))
    }

    async fn set_direction(
        &mut self,
        mask: u32,
        dir: Direction,
        state: bool,
    ) -> Result<(), Error<I>> {
        // set state before switching direction to prevent glitch
        if dir == crate::Direction::Output {
            if state {
                self.set(mask, 0).await?;
            } else {
                self.set(0, mask).await?;
            }
        }

        let (mask_set, mask_clear) = match dir {
            crate::Direction::Input => (mask as u16, 0),
            crate::Direction::Output => (0, mask as u16),
        };
        if mask & 0x00FF != 0 {
            self.update_reg(
                Reg::Configuration0,
                (mask_set & 0xFF) as u8,
                (mask_clear & 0xFF) as u8,
            )
            .await?;
        }
        if mask & 0xFF00 != 0 {
            self.update_reg(
                Reg::Configuration1,
                (mask_set >> 8) as u8,
                (mask_clear >> 8) as u8,
            )
            .await?;
        }
        Ok(())
    }

    async fn set_polarity(&mut self, mask: u32, inverted: bool) -> Result<(), Error<I>> {
        let (mask_set, mask_clear) = match inverted {
            false => (0, mask as u16),
            true => (mask as u16, 0),
        };

        if mask & 0x00FF != 0 {
            self.update_reg(
                Reg::PolarityInversion0,
                (mask_set & 0xFF) as u8,
                (mask_clear & 0xFF) as u8,
            )
            .await?;
        }
        if mask & 0xFF00 != 0 {
            self.update_reg(
                Reg::PolarityInversion1,
                (mask_set >> 8) as u8,
                (mask_clear >> 8) as u8,
            )
            .await?;
        }
        Ok(())
    }
}
