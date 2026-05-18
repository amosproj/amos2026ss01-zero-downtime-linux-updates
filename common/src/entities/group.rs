pub mod group {
    // use sea_orm::entity::prelude::*;
	use sea_orm::*;
	// use crate::entities::Device;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Default)]
    #[sea_orm(table_name = "groups")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = true)]
        pub id: i32,
        pub name: String,
		// #[sea_orm(has_many)]
    	// pub device: HasMany<crate::entities::device::device::Entity>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
	pub enum Relation {
		#[sea_orm(has_many = "crate::entities::device::device::Entity")]
		Device,
	}

	impl Related<crate::entities::device::device::Entity> for Entity {
		fn to() -> RelationDef {
			Relation::Device.def()
		}
	}

    impl ActiveModelBehavior for ActiveModel {}
}
