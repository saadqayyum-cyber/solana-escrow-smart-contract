use anchor_lang::prelude::*;

declare_id!("DpUanPzBTt89ZWfaotgc1QRJTpGWMSr51uohFktcrsTb");
//program id: DpUanPzBTt89ZWfaotgc1QRJTpGWMSr51uohFktcrsTb

#[program]
pub mod smart_contract {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

}

#[derive(Accounts)]
pub struct Initialize {}