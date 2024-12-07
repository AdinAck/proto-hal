use proto_hal::macros::block;

#[block(
    base_addr = 0x4002_0c00,
    auto_increment,
    entitlements = [super::rcc::ahb1enr::cordicen::Enabled],
    erase_mod,
)]
mod cordic {
    #[register(auto_increment)]
    mod csr {
        #[field(width = 4, read, write, auto_increment)]
        mod func {
            #[state(entitlements = [scale::N0], reset)]
            struct Cos;

            #[state(entitlements = [scale::N0])]
            struct Sin;

            #[state(entitlements = [scale::N0])]
            struct ATan2;

            #[state(entitlements = [scale::N0])]
            struct Magnitude;

            #[state]
            struct ATan;

            #[state(entitlements = [scale::N1])]
            struct CosH;

            #[state(entitlements = [scale::N1])]
            struct SinH;

            #[state(entitlements = [scale::N1])]
            struct ATanH;

            #[state(entitlements = [scale::N1, scale::N2, scale::N3, scale::N4])]
            struct Ln;

            #[state(entitlements = [scale::N0, scale::N1, scale::N2])]
            struct Sqrt;
        }

        #[field(width = 4, read, write, auto_increment)]
        /// custom docs
        mod precision {
            #[state(bits = 1)]
            struct P4;
            #[state]
            struct P8;
            #[state]
            struct P12;
            #[state]
            struct P16;
            #[state(reset)]
            struct P20;
            #[state]
            struct P24;
            #[state]
            struct P28;
            #[state]
            struct P32;
            #[state]
            struct P36;
            #[state]
            struct P40;
            #[state]
            struct P44;
            #[state]
            struct P48;
            #[state]
            struct P52;
            #[state]
            struct P56;
            #[state]
            struct P60;
        }

        #[field(width = 3, read, write, auto_increment)]
        mod scale {
            #[state(reset)]
            struct N0;
            #[state]
            struct N1;
            #[state]
            struct N2;
            #[state]
            struct N3;
            #[state]
            struct N4;
            #[state]
            struct N5;
            #[state]
            struct N6;
            #[state]
            struct N7;
        }

        #[field(offset = 16, width = 1, read, write)]
        mod ien {
            #[state(reset, bits = 0)]
            struct Disabled;
            #[state(bits = 1)]
            struct Enabled;
        }

        #[field(width = 1, read, write)]
        mod dmaren {
            #[state(reset, bits = 0)]
            struct Disabled;
            #[state(bits = 1)]
            struct Enabled;
        }

        #[field(width = 1, read, write)]
        mod dmawen {
            #[state(reset, bits = 0)]
            struct Disabled;
            #[state(bits = 1)]
            struct Enabled;
        }

        #[field(width = 1, read, write)]
        mod nres {
            #[state(reset, bits = 0)]
            struct OneRead;
            #[state(bits = 1, entitlements = [ressize::Q31])]
            struct TwoReads;
        }

        #[field(width = 1, read, write)]
        mod nargs {
            #[state(reset, bits = 0)]
            struct OneWrite;
            #[state(bits = 1, entitlements = [argsize::Q31])]
            struct TwoWrites;
        }

        #[field(width = 1, read, write)]
        mod ressize {
            #[state(reset, bits = 0)]
            struct Q31;
            #[state(bits = 1)]
            struct Q15;
        }

        #[field(width = 1, read, write)]
        mod argsize {
            #[state(reset, bits = 0)]
            struct Q31;
            #[state(bits = 1)]
            struct Q15;
        }

        #[field(offset = 31, width = 1, read)]
        mod rrdy {
            #[state(reset, bits = 0)]
            struct NoData;
            #[state(bits = 1)]
            struct DataReady;
        }
    }

    #[register]
    mod wdata {
        #[field(offset = 0, width = 32, write(effect = unresolve(csr::rrdy)))]
        mod arg {}
    }

    #[register]
    mod rdata {
        #[field(offset = 0, width = 32, reset = 0, read(entitlements = [csr::rrdy::Ready], effect = unresolve(csr::rrdy)))]
        mod res {}
    }
}
