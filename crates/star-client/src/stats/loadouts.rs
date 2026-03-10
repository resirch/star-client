use crate::riot::api::RiotApiClient;
use crate::riot::types::LoadoutsResponse;

const WEAPON_UUIDS: &[(&str, &str)] = &[
    ("vandal", "9c82e19d-4575-0200-1a81-3eacf00cf872"),
    ("phantom", "ee8e8d15-496b-07ac-f604-8f8488911e76"),
    ("operator", "a03b24d3-4319-996d-0f8c-94bbfba1dfc7"),
    ("sheriff", "e336c6b8-418d-9340-d77f-7a9e4cfe0702"),
    ("spectre", "462080d1-4035-2937-7c09-27aa2a5c27a7"),
    ("classic", "29a0cfab-485b-f5d5-779a-b59f85e204a8"),
    ("ghost", "1baa85b4-4c70-1284-64bb-6481dfc3bb4e"),
    ("frenzy", "44d4e95c-4157-0037-81b2-17841bf2e8e3"),
    ("stinger", "f7e1b454-4ad4-1063-ec0a-159e56b58941"),
    ("marshal", "c4883e50-4494-202c-3ec3-6b8a9284f00b"),
    ("guardian", "4ade7faa-4cf1-8376-95ef-39884480959b"),
    ("bulldog", "ae3de142-4d85-2547-dd26-4e90bed35cf7"),
    ("ares", "55d8a0f4-4274-ca67-fe2c-06ab45ac8571"),
    ("odin", "63e6c2b6-4a8e-869c-3d4c-e38355226584"),
    ("judge", "ec845bf4-4f79-ddda-a3da-0db3774b2794"),
    ("bucky", "910be174-449b-c412-ab22-d0873436b21b"),
    ("shorty", "42da8ccc-40d5-affc-beec-15aa47b42eda"),
];

pub fn weapon_uuid(name: &str) -> &str {
    WEAPON_UUIDS
        .iter()
        .find(|(n, _)| *n == name.to_lowercase())
        .map(|(_, u)| *u)
        .unwrap_or(WEAPON_UUIDS[0].1)
}

pub fn extract_skin(
    api: &RiotApiClient,
    loadouts: &LoadoutsResponse,
    puuid: &str,
    weapon_name: &str,
) -> String {
    let weapon_id = weapon_uuid(weapon_name);

    for loadout in &loadouts.loadouts {
        if loadout.subject != puuid {
            continue;
        }
        if let Some(items) = &loadout.items {
            if let Some(item) = items.get(weapon_id) {
                if let Some(sockets) = &item.sockets {
                    for socket in sockets.values() {
                        if let Some(socket_item) = &socket.item {
                            if let Some(id) = &socket_item.i_d {
                                let name = api.get_skin_name(id);
                                if name != "Unknown" {
                                    return name;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    "Standard".to_string()
}
