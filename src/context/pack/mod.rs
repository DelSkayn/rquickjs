pub trait Packable {
    type Package;

    fn runtime_id(&self) -> RuntimeId {}

    fn pack(ctx: Ctx<'js>) -> Package;

    fn unpack(pack: Self::Package, ctx: Ctx<'js>) -> Self;
}

pub struct ObjectPack {}
