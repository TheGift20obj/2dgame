
use bevy::prelude::*;
use bevy::app::AppExit;
use crate::physics_resources::*;

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct MenuCamera;


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuButtonAction {
    NewGame,
    LoadGame,
    Options,
    Exit,
}

#[derive(Component, Clone, Copy)]
pub struct MenuButton(pub MenuButtonAction);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameStatus(false))
            .add_systems(Startup, init)
           .add_systems(Update, button_system);
    }
}

fn init(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_ui(&mut commands, &asset_server);
    commands.spawn((
        Camera2d,
        MenuCamera
    ));
}

pub fn setup_ui(commands: &mut Commands, asset_server: &Res<AssetServer>) {

    // font used by buttons
    let font = asset_server.load("fonts/Cantarell-Bold.ttf");

    // root node: full-screen, centered column
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            padding: UiRect::top(Val::Percent(13.5)),
            ..default()
        },
        ImageNode::new(asset_server.load("textures/menu.png")),
        MenuRoot,
    ))
    .with_children(|parent| {
        // optional spacer / logo area
        parent.spawn((
            Node { margin: UiRect::bottom(Val::Px(20.0)), ..default() },
        ));

        // buttons (stacked vertically)
        parent.spawn((
            Node { margin: UiRect::bottom(Val::Px(20.0)), ..default() },
        ));
        parent.spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(60.0),
                margin: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            BorderColor(Color::BLACK.into()),
            MenuButton(MenuButtonAction::NewGame),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("New Game"),
                TextFont { font: font.clone(), font_size: 28.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
        parent.spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(60.0),
                margin: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            BorderColor(Color::BLACK.into()),
            MenuButton(MenuButtonAction::LoadGame),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Load Game"),
                TextFont { font: font.clone(), font_size: 28.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
        parent.spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(60.0),
                margin: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            BorderColor(Color::BLACK.into()),
            MenuButton(MenuButtonAction::Options),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Options"),
                TextFont { font: font.clone(), font_size: 28.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
        parent.spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(60.0),
                margin: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            BorderColor(Color::BLACK.into()),
            MenuButton(MenuButtonAction::Exit),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Exit"),
                TextFont { font: font.clone(), font_size: 28.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
    });
}

fn button_system(
    mut commands: Commands,
    mut interaction_query: Query<(&Interaction, &mut BackgroundColor, Option<&MenuButton>), (Changed<Interaction>, With<Button>)>,
    menu_root_query: Query<Entity, With<MenuRoot>>,
    mut exit: EventWriter<AppExit>,
    mut game_status: ResMut<GameStatus>,
    camera_query: Query<Entity, With<MenuCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    images: Res<Assets<Image>>,
    config: Res<ItemConfig>,
    atlas_handles: Res<AtlasHandles>,
) {
    for (interaction, mut bg_color, menu_button) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = PRESSED_BUTTON.into();
                if let Some(btn) = menu_button {
                    match btn.0 {
                        MenuButtonAction::NewGame => {
                            // close initial menu (despawn all MenuRoot nodes)
                            for root in menu_root_query.iter() {
                                commands.entity(root).despawn_recursive();
                                for entity in camera_query {
                                    commands.entity(entity).despawn_recursive();
                                }
                                crate::player_game_ui::spawn_health_bar(&mut commands, &asset_server);
                                crate::player_game_ui::spawn_inventory_bar(&mut commands, &asset_server);
                                crate::player::init(&mut commands, &mut meshes, &mut materials, &asset_server, &mut texture_atlas_layouts, &images, &config, &atlas_handles);
                                game_status.0 = true;
                            }
                        }
                        MenuButtonAction::Exit => {
                            // close the app
                            exit.send(AppExit::Success);
                        }
                        MenuButtonAction::LoadGame => {
                            // no-op (placeholder)
                        }
                        MenuButtonAction::Options => {
                            // no-op (placeholder)
                        }
                    }
                }
            }
            Interaction::Hovered => {
                *bg_color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                *bg_color = NORMAL_BUTTON.into();
            }
        }
    }
}
