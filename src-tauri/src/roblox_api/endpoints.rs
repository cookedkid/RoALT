//! Every Roblox URL in one place (verified 2026-10-24).

pub const USERS_AUTHENTICATED: &str = "https://users.roblox.com/v1/users/authenticated";
pub const LOGIN_PAGE: &str = "https://www.roblox.com/login";
pub const WWW: &str = "https://www.roblox.com/";
/// `Origin` the website sends to auth.roblox.com and apis.roblox.com (`Referer` is `WWW`).
pub const WEB_ORIGIN: &str = "https://www.roblox.com";

// Account info. `{}` is filled by `fill`; each template is also its endpoint's throttle key.
pub const USER: &str = "https://users.roblox.com/v1/users/{}";
pub const HEADSHOTS: &str =
    "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png&isCircular=false";
pub const CURRENCY: &str = "https://economy.roblox.com/v1/user/currency";
pub const PREMIUM: &str = "https://premiumfeatures.roblox.com/v1/users/{}/validate-membership";
pub const PROFILES: &str = "https://apis.roblox.com/user-profile-api/v1/user/profiles/get-profiles";
pub const FRIENDS_COUNT: &str = "https://friends.roblox.com/v1/users/{}/friends/count";
pub const FOLLOWERS_COUNT: &str = "https://friends.roblox.com/v1/users/{}/followers/count";
pub const FOLLOWINGS_COUNT: &str = "https://friends.roblox.com/v1/users/{}/followings/count";
pub const PRESENCE: &str = "https://presence.roblox.com/v1/presence/users";
pub const EMAIL: &str = "https://accountsettings.roblox.com/v1/email";
pub const TWO_STEP: &str = "https://twostepverification.roblox.com/v1/users/{}/configuration";

// Launching. Trailing slashes as the website sends them.
pub const CLIENT_ASSERTION: &str = "https://auth.roblox.com/v1/client-assertion/";
pub const AUTH_TICKET: &str = "https://auth.roblox.com/v1/authentication-ticket/";
pub const SHARE_LINK: &str = "https://apis.roblox.com/sharelinks/v1/resolve-link";
pub const SHARE_LINK_V2: &str = "https://apis.roblox.com/deeplinks/v2/resolve";
pub const GAMES: &str = "https://games.roblox.com/v1/games?universeIds={}";
pub const USERS_BY_NAME: &str = "https://users.roblox.com/v1/usernames/users";
pub const FRIENDS_ONLINE: &str = "https://friends.roblox.com/v1/users/{}/friends/online";
/// What the client asks before it joins a game (cookie + CSRF); unrecorded signed in.
pub const JOIN_GAME: &str = "https://gamejoin.roblox.com/v1/join-game";

// Friending stored accounts with each other. Statuses is in the public spec (Experimental); the
// two POSTs are no longer listed there but are what the website calls.
pub const FRIEND_STATUSES: &str = "https://friends.roblox.com/v1/users/{}/friends/statuses";
pub const REQUEST_FRIENDSHIP: &str = "https://friends.roblox.com/v1/users/{}/request-friendship";
pub const ACCEPT_FRIEND: &str = "https://friends.roblox.com/v1/users/{}/accept-friend-request";

// Servers (query string appended by the caller).
pub const SERVERS_PUBLIC: &str = "https://games.roblox.com/v2/games/{}/servers/Public";
pub const SERVERS_PRIVATE: &str = "https://games.roblox.com/v1/games/{}/private-servers";
// Private servers, as their owner (cookie + CSRF). Shapes from the public spec, unrecorded signed in.
pub const VIP_SERVER: &str = "https://games.roblox.com/v1/vip-servers/{}";
pub const VIP_SERVER_PERMISSIONS: &str = "https://games.roblox.com/v1/vip-servers/{}/permissions";
pub const VIP_SERVER_SUBSCRIPTION: &str = "https://games.roblox.com/v1/vip-servers/{}/subscription";
/// Buys a private server of a universe: spends Robux.
pub const VIP_SERVER_CREATE: &str = "https://games.roblox.com/v1/games/vip-servers/{}";
pub const PLACE_UNIVERSE: &str = "https://apis.roblox.com/universes/v1/places/{}/universe";
pub const PLACE_ICONS: &str =
    "https://thumbnails.roblox.com/v1/places/gameicons?placeIds={}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false";

// Browse's Popular (no cookie): the Charts' lists, and game icons by universe.
pub const EXPLORE_SORTS: &str = "https://apis.roblox.com/explore-api/v1/get-sorts?sessionId={}&device=computer";
pub const GAME_ICONS: &str = "https://thumbnails.roblox.com/v1/games/icons?universeIds={}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false";

// A game's details (no cookie, verified 2026-11-08). Paging is appended by the caller: passes
// `&pageToken=`, badges and collectibles `&cursor=`. (games.roblox.com's own game-passes list is gone: 404.)
pub const GAME_VOTES: &str = "https://games.roblox.com/v1/games/votes?universeIds={}";
pub const GAME_PASSES: &str = "https://apis.roblox.com/game-passes/v1/universes/{}/game-passes?passView=Full&pageSize=100";
pub const PASS_ICONS: &str = "https://thumbnails.roblox.com/v1/game-passes?gamePassIds={}&size=150x150&format=Png";
pub const GAME_BADGES: &str = "https://badges.roblox.com/v1/universes/{}/badges?limit=100&sortOrder=Asc";
/// Browse › Limiteds' item pictures (no cookie, verified 2026-11-09).
pub const ASSET_THUMBS: &str = "https://thumbnails.roblox.com/v1/assets?assetIds={}&returnPolicy=PlaceHolder&size=110x110&format=Webp&isCircular=false";
pub const BADGE_ICONS: &str = "https://thumbnails.roblox.com/v1/badges/icons?badgeIds={}&size=150x150&format=Png";
/// An account's limiteds, one row per copy, with their RAP (`recentAveragePrice`). Sent with the
/// account's own cookie: it sees a private inventory, and the limit is per user, not per address.
pub const COLLECTIBLES: &str = "https://inventory.roblox.com/v1/users/{}/assets/collectibles?limit=100&sortOrder=Asc";
/// More of a game (no cookie, verified 2026-11-08): its age rating (POST `{universeId}`), its
/// universe (`isArchived`) and places, and the group that owns it (7 a minute: only inside the
/// cached `game_stats`).
pub const AGE_RATING: &str = "https://apis.roblox.com/experience-guidelines-api/experience-guidelines/get-age-recommendation";
pub const UNIVERSE: &str = "https://develop.roblox.com/v1/universes/{}";
pub const UNIVERSE_PLACES: &str = "https://develop.roblox.com/v1/universes/{}/places?limit=100";
pub const GROUP: &str = "https://groups.roblox.com/v1/groups/{}";

// A player's profile (no cookie, verified 2026-11-08). Their badge list, Premium and username
// history need a cookie or allow 1 call a minute: not asked.
pub const USER_GROUPS: &str = "https://groups.roblox.com/v2/users/{}/groups/roles";
pub const ROBLOX_BADGES: &str = "https://accountinformation.roblox.com/v1/users/{}/roblox-badges";
/// `limit` must be 10, 25 or 50.
pub const USER_GAMES: &str = "https://games.roblox.com/v2/users/{}/games?accessFilter=Public&limit=50&sortOrder=Desc";

// Outfits (the account's cookie; writes need CSRF). The spec marks these v1–v3 calls deprecated,
// but all answered on 2026-11-09; v4's one write call takes `updateTypes` numbers the spec doesn't explain.
/// An account's own outfits. The caller appends `&paginationToken=`.
pub const OUTFITS: &str = "https://avatar.roblox.com/v2/avatar/users/{}/outfits?isEditable=true&itemsPerPage=50";
/// As its owner: another account can get a 403.
pub const OUTFIT_DETAILS: &str = "https://avatar.roblox.com/v3/outfits/{}/details";
/// What the account wears now.
pub const AVATAR: &str = "https://avatar.roblox.com/v2/avatar/avatar";
pub const SET_AVATAR_TYPE: &str = "https://avatar.roblox.com/v1/avatar/set-player-avatar-type";
pub const SET_BODY_COLORS: &str = "https://avatar.roblox.com/v2/avatar/set-body-colors";
pub const SET_SCALES: &str = "https://avatar.roblox.com/v1/avatar/set-scales";
pub const SET_WEARING_ASSETS: &str = "https://avatar.roblox.com/v2/avatar/set-wearing-assets";
pub const OUTFIT_CREATE: &str = "https://avatar.roblox.com/v3/outfits/create";
/// PATCH: a partial update (only `name` is sent).
pub const OUTFIT: &str = "https://avatar.roblox.com/v3/outfits/{}";
pub const OUTFIT_DELETE: &str = "https://avatar.roblox.com/v1/outfits/{}/delete";
/// Pictures of outfits and of whole avatars (no cookie, verified 2026-11-09).
pub const OUTFIT_THUMBS: &str = "https://thumbnails.roblox.com/v1/users/outfits?userOutfitIds={}&size=150x150&format=Png&isCircular=false";
pub const AVATAR_THUMBS: &str = "https://thumbnails.roblox.com/v1/users/avatar?userIds={}&size=150x150&format=Png&isCircular=false";

// Trades (the account's cookie; accept and decline need CSRF). Trades API v1, not deprecated.
/// `{}`: Inbound, Outbound or Completed. One page: the newest 100.
pub const TRADES: &str = "https://trades.roblox.com/v1/trades/{}?limit=100&sortOrder=Desc";
pub const TRADE: &str = "https://trades.roblox.com/v1/trades/{}";
pub const TRADE_ACCEPT: &str = "https://trades.roblox.com/v1/trades/{}/accept";
/// Also cancels a trade the account sent.
pub const TRADE_DECLINE: &str = "https://trades.roblox.com/v1/trades/{}/decline";

// Friend requests an account got (its cookie; the POSTs need CSRF). All answered signed out on
// 2026-11-11 (401, the POSTs 403; a made-up path 404). Shapes from the public spec, unrecorded signed in.
/// The newest 100.
pub const FRIEND_REQUESTS: &str = "https://friends.roblox.com/v1/my/friends/requests?limit=100&sortOrder=Desc";
pub const DECLINE_FRIEND: &str = "https://friends.roblox.com/v1/users/{}/decline-friend-request";
pub const DECLINE_ALL_FRIENDS: &str = "https://friends.roblox.com/v1/user/friend-requests/decline-all";
/// Names by id: POST `{userIds, excludeBannedUsers}`, no cookie, 100 ids at most (verified 2026-11-11).
pub const USERS_BY_ID: &str = "https://users.roblox.com/v1/users";

// An account's Robux history (its cookie), as the website's Transactions page reads it. Answered
// signed out on 2026-11-11 (401; a made-up path 404); shapes unrecorded signed in, so read leniently.
/// The newest 25 of one kind: the caller appends the kind (`RobuxKind::path`).
pub const TRANSACTIONS: &str = "https://apis.roblox.com/transaction-records/v1/users/{}/transactions?limit=25&itemPricingType=All&transactionType=";
/// The last month's totals.
pub const TRANSACTION_TOTALS: &str = "https://apis.roblox.com/transaction-records/v1/users/{}/transaction-totals?timeFrame=Month&transactionType=summary";

/// The private servers an account owns (its cookie; 401 signed out on 2026-11-11). The caller appends `&cursor=`.
pub const MY_PRIVATE_SERVERS: &str = "https://games.roblox.com/v1/private-servers/my-private-servers?privateServerResultsFilter=MyPrivateServers&itemsPerPage=100";

/// `template` with its `{}` replaced by `value`.
pub fn fill(template: &str, value: impl std::fmt::Display) -> String {
    template.replacen("{}", &value.to_string(), 1)
}

// Roblox client updates (no cookie). The installer URL is `{SETUP}{clientVersionUpload}-RobloxPlayerInstaller.exe`.
pub const CLIENT_VERSION: &str = "https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer";
pub const SETUP: &str = "https://setup.rbxcdn.com/";
