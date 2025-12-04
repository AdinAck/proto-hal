use proto_hal_build::model::{Model, interrupts::Interrupt};

use crate::{cordic::cordic, crc::crc, rcc::rcc};

pub mod cordic;
pub mod crc;
pub mod rcc;

#[derive(Debug)]
pub enum DeviceVariant {
    G431,
    G441,
    G474,
    G484,
}

pub fn model(variant: DeviceVariant) -> Model {
    let extra_interrupts = |interrupt| {
        if matches!(variant, DeviceVariant::G474 | DeviceVariant::G484) {
            interrupt
        } else {
            Interrupt::reserved()
        }
    };

    let mut model = Model::new().with_interrupts([
        Interrupt::handler("WWDG").docs(["Window Watchdog"]),
        Interrupt::handler("PVD_PVM").docs(["PVD through EXTI line detection"]),
        Interrupt::handler("RTC_TAMP_CSS_LSE"),
        Interrupt::handler("RTC_WKUP").docs(["RTC Wakeup timer"]),
        Interrupt::handler("FLASH"),
        Interrupt::handler("RCC"),
        Interrupt::handler("EXTI0"),
        Interrupt::handler("EXTI1"),
        Interrupt::handler("EXTI2"),
        Interrupt::handler("EXTI3"),
        Interrupt::handler("EXTI4"),
        Interrupt::handler("DAM1_CH1"),
        Interrupt::handler("DAM1_CH2"),
        Interrupt::handler("DAM1_CH3"),
        Interrupt::handler("DAM1_CH4"),
        Interrupt::handler("DAM1_CH5"),
        Interrupt::handler("DAM1_CH6"),
        extra_interrupts(Interrupt::handler("DAM1_CH7")),
        Interrupt::handler("ADC1_2").docs(["ADC1 and ADC2 global interrupt"]),
        Interrupt::handler("USB_HP"),
        Interrupt::handler("USB_LP"),
        Interrupt::handler("FDCAN1_INTR0_IT"),
        Interrupt::handler("FDCAN1_INTR1_IT"),
        Interrupt::handler("EXTI9_5").docs(["EXTI lanes 5 through 9"]),
        Interrupt::handler("TIM1_BRK_TIM15"),
        Interrupt::handler("TIM1_UP_TIM16"),
        Interrupt::handler("TIM1_TRG_COM"),
        Interrupt::handler("TIM1_CC"),
        Interrupt::handler("TIM2"),
        Interrupt::handler("TIM3"),
        Interrupt::handler("TIM4"),
        Interrupt::handler("I2C1_EV"),
        Interrupt::handler("I2C1_ER"),
        Interrupt::handler("I2C2_EV"),
        Interrupt::handler("I2C2_ER"),
        Interrupt::handler("SPI1"),
        Interrupt::handler("SPI2"),
        Interrupt::handler("USART1"),
        Interrupt::handler("USART2"),
        Interrupt::handler("USART3"),
        Interrupt::handler("EXTI15_10").docs(["EXTI lanes 10 through 15"]),
        Interrupt::handler("RTC_ALARM"),
        Interrupt::handler("USBWAKE_UP"),
        Interrupt::handler("TIM8_BRK"),
        Interrupt::handler("TIM8_UP"),
        Interrupt::handler("TIM8_TRG_COM"),
        Interrupt::handler("TIM8_CC"),
        extra_interrupts(Interrupt::handler("ADC3")),
        extra_interrupts(Interrupt::handler("FMC")),
        Interrupt::handler("LPTIM1"),
        extra_interrupts(Interrupt::handler("TIM5")),
        Interrupt::handler("SPI3"),
        Interrupt::handler("UART4"),
        extra_interrupts(Interrupt::handler("UART5")),
        Interrupt::handler("TIM6_DACUNDER"),
        Interrupt::handler("TIM7"),
        Interrupt::handler("DMA2_CH1"),
        Interrupt::handler("DMA2_CH2"),
        Interrupt::handler("DMA2_CH3"),
        Interrupt::handler("DMA2_CH4"),
        Interrupt::handler("DMA2_CH5"),
        extra_interrupts(Interrupt::handler("ADC4")),
        extra_interrupts(Interrupt::handler("ADC5")),
        Interrupt::handler("UCPD1"),
        Interrupt::handler("COMP1_2_3"),
        Interrupt::handler("COMP4_5_6"),
        extra_interrupts(Interrupt::handler("COMP7")),
        extra_interrupts(Interrupt::handler("HRTIM_MASTER_IRQN")),
        extra_interrupts(Interrupt::handler("HRTIM_TIMA_IRQN")),
        extra_interrupts(Interrupt::handler("HRTIM_TIMB_IRQN")),
        extra_interrupts(Interrupt::handler("HRTIM_TIMC_IRQN")),
        extra_interrupts(Interrupt::handler("HRTIM_TIMD_IRQN")),
        extra_interrupts(Interrupt::handler("HRTIM_TIME_IRQN")),
        extra_interrupts(Interrupt::handler("HRTIM_TIM_FLT_IRQN")),
        extra_interrupts(Interrupt::handler("HRTIM_TIMF_IRQN")),
        Interrupt::handler("CRS"),
        Interrupt::handler("SAI"),
        extra_interrupts(Interrupt::handler("TIM20_BRK")),
        extra_interrupts(Interrupt::handler("TIM20_UP")),
        extra_interrupts(Interrupt::handler("TIM20_TRG_COM")),
        extra_interrupts(Interrupt::handler("TIM20_CC")),
        Interrupt::handler("FPU"),
        extra_interrupts(Interrupt::handler("I2C4_EV")),
        extra_interrupts(Interrupt::handler("I2C4_ER")),
        extra_interrupts(Interrupt::handler("SPI4")),
        Interrupt::handler("AES"),
        extra_interrupts(Interrupt::handler("FDCAN2_INTR0")),
        extra_interrupts(Interrupt::handler("FDCAN2_INTR1")),
        extra_interrupts(Interrupt::handler("FDCAN3_INTR0")),
        extra_interrupts(Interrupt::handler("FDCAN3_INTR1")),
        Interrupt::handler("RNG"),
        Interrupt::handler("LPUART"),
        Interrupt::handler("I2C3_EV"),
        Interrupt::handler("I2C3_ER"),
        Interrupt::handler("DMAMUX_OVR"),
        extra_interrupts(Interrupt::handler("QUADSPI")),
        extra_interrupts(Interrupt::handler("DMA1_CH8")),
        Interrupt::handler("DMA2_CH6"),
        extra_interrupts(Interrupt::handler("DMA2_CH7")),
        extra_interrupts(Interrupt::handler("DMA2_CH8")),
        Interrupt::handler("CORDIC"),
        Interrupt::handler("FMAC"),
    ]);

    let rcc::Output { cordicen, crcen } = rcc(&mut model);
    cordic(&mut model, cordicen);
    crc(&mut model, crcen);

    model
}
