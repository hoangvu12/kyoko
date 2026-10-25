use bridge::{Drop, rearranged, rearranged_folder};
use protocol::Snowflake;
use state::RailEntry;

const NEW_FOLDER: u64 = 77;

fn g(id: u64) -> RailEntry {
	RailEntry::Guild(Snowflake(id))
}

fn folder(id: u64, guilds: &[u64]) -> RailEntry {
	RailEntry::Folder {
		id,
		name: Some(format!("folder {id}")),
		color: Some(0x123456),
		guilds: guilds.iter().copied().map(Snowflake).collect(),
	}
}

fn layout() -> Vec<RailEntry> {
	vec![g(1), folder(10, &[2, 3]), g(4), g(5)]
}

fn moved(guild: u64, drop: Drop) -> Option<Vec<RailEntry>> {
	rearranged(&layout(), Snowflake(guild), drop, NEW_FOLDER)
}

#[test]
fn a_guild_moves_before_another_guild() {
	assert_eq!(
		moved(5, Drop::BeforeGuild(Snowflake(1))),
		Some(vec![g(5), g(1), folder(10, &[2, 3]), g(4)])
	);
}

#[test]
fn a_guild_moves_to_the_end() {
	assert_eq!(
		moved(1, Drop::End),
		Some(vec![folder(10, &[2, 3]), g(4), g(5), g(1)])
	);
}

#[test]
fn a_guild_moves_before_a_folder() {
	assert_eq!(
		moved(4, Drop::BeforeFolder(10)),
		Some(vec![g(1), g(4), folder(10, &[2, 3]), g(5)])
	);
}

#[test]
fn dropping_onto_a_guild_makes_a_new_folder_in_its_place() {
	assert_eq!(
		moved(5, Drop::OntoGuild(Snowflake(1))),
		Some(vec![
			RailEntry::Folder {
				id: NEW_FOLDER,
				name: None,
				color: None,
				guilds: vec![Snowflake(1), Snowflake(5)],
			},
			folder(10, &[2, 3]),
			g(4),
		])
	);
}

#[test]
fn dropping_onto_a_folder_appends_to_it() {
	assert_eq!(
		moved(4, Drop::OntoFolder(10)),
		Some(vec![g(1), folder(10, &[2, 3, 4]), g(5)])
	);
}

#[test]
fn a_guild_moves_inside_its_folder() {
	assert_eq!(
		moved(3, Drop::BeforeGuild(Snowflake(2))),
		Some(vec![g(1), folder(10, &[3, 2]), g(4), g(5)])
	);
}

#[test]
fn a_guild_leaves_its_folder_and_the_last_one_out_removes_it() {
	let once = moved(2, Drop::End).expect("the guild leaves the folder");
	assert_eq!(once, vec![g(1), folder(10, &[3]), g(4), g(5), g(2)]);
	let twice = rearranged(&once, Snowflake(3), Drop::BeforeGuild(Snowflake(1)), NEW_FOLDER);
	assert_eq!(twice, Some(vec![g(3), g(1), g(4), g(5), g(2)]));
}

#[test]
fn dropping_onto_a_guild_inside_a_folder_lands_beside_it() {
	assert_eq!(
		moved(5, Drop::OntoGuild(Snowflake(3))),
		Some(vec![g(1), folder(10, &[2, 5, 3]), g(4)])
	);
}

#[test]
fn drops_that_change_nothing_are_refused() {
	assert_eq!(moved(1, Drop::OntoGuild(Snowflake(1))), None);
	assert_eq!(moved(1, Drop::BeforeGuild(Snowflake(1))), None);
	assert_eq!(moved(4, Drop::BeforeGuild(Snowflake(5))), None);
	assert_eq!(moved(5, Drop::End), None);
	assert_eq!(moved(9, Drop::End), None);
	assert_eq!(moved(1, Drop::OntoFolder(99)), None);
}

#[test]
fn a_folder_moves_before_a_guild_or_to_the_end() {
	assert_eq!(
		rearranged_folder(&layout(), 10, Drop::BeforeGuild(Snowflake(1))),
		Some(vec![folder(10, &[2, 3]), g(1), g(4), g(5)])
	);
	assert_eq!(
		rearranged_folder(&layout(), 10, Drop::End),
		Some(vec![g(1), g(4), g(5), folder(10, &[2, 3])])
	);
}

#[test]
fn a_folder_never_goes_into_a_folder() {
	let nested = vec![folder(10, &[2]), folder(20, &[3])];
	assert_eq!(rearranged_folder(&nested, 10, Drop::OntoFolder(20)), None);
	assert_eq!(
		rearranged_folder(&nested, 10, Drop::OntoGuild(Snowflake(3))),
		None
	);
	assert_eq!(rearranged_folder(&nested, 10, Drop::BeforeFolder(10)), None);
	assert_eq!(
		rearranged_folder(&nested, 20, Drop::BeforeGuild(Snowflake(2))),
		Some(vec![folder(20, &[3]), folder(10, &[2])])
	);
}
