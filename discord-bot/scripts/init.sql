BEGIN;

CREATE TYPE public.lobby_privacy AS ENUM
    ('open', 'invite_only');

CREATE TABLE IF NOT EXISTS public.matchmaking_lobbies
(
    id uuid NOT NULL,
    started_at timestamp with time zone NOT NULL,
    timeout_after timestamp with time zone NOT NULL,
    channel_id bigint NOT NULL,
    description character varying(255) COLLATE pg_catalog."default",
    owner uuid NOT NULL,
    privacy lobby_privacy NOT NULL,
    game uuid,
    game_other character varying(80) DEFAULT 'Other',
    CONSTRAINT matchmaking_lobbies_pkey PRIMARY KEY (id),
    CONSTRAINT matchmaking_lobbies_unique UNIQUE (id)
);

CREATE TABLE IF NOT EXISTS public.matchmaking_settings
(
    guild_id bigint NOT NULL,
    has_accepted_eula timestamp with time zone,
    last_updated timestamp with time zone NOT NULL DEFAULT '2022-10-01 21:49:54+00'::timestamp with time zone,
    channel_id bigint,
    admin_role bigint,
    threads_are_private boolean DEFAULT false,
    CONSTRAINT matchmaking_settings_pkey PRIMARY KEY (guild_id),
    CONSTRAINT matchmaking_settings_unique UNIQUE (guild_id)
);

CREATE TABLE IF NOT EXISTS public.seaql_migrations
(
    version character varying COLLATE pg_catalog."default" NOT NULL,
    applied_at bigint NOT NULL,
    CONSTRAINT seaql_migrations_pkey PRIMARY KEY (version)
);

CREATE TABLE IF NOT EXISTS public.users
(
    user_id uuid NOT NULL,
    discord_user bigint,
    CONSTRAINT users_pkey PRIMARY KEY (user_id),
    CONSTRAINT users_unique_discord_id UNIQUE (discord_user),
    CONSTRAINT users_unique_user_id UNIQUE (user_id)
);

CREATE TABLE IF NOT EXISTS public.matchmaking_player_lobby
(
    player uuid NOT NULL,
    lobby uuid NOT NULL,
    "character" uuid,
    character_other character varying(80),
    joined_at timestamp with time zone NOT NULL,
    CONSTRAINT matchmaking_player_lobby_pkey PRIMARY KEY (player, lobby),
    CONSTRAINT matchmaking_player_lobby_unique UNIQUE (player, lobby)
);

COMMENT ON TABLE public.matchmaking_player_lobby
    IS 'The players that are playing in the lobby.';

CREATE TABLE IF NOT EXISTS public.game_character
(
    id uuid NOT NULL,
    name character varying(80) NOT NULL,
    game uuid NOT NULL,
    CONSTRAINT game_character_pkey PRIMARY KEY (id),
    CONSTRAINT game_character_unique UNIQUE (id),
    CONSTRAINT game_character_unique_name UNIQUE (name)
);

COMMENT ON TABLE public.game_character
    IS 'A character from a specific game.';

CREATE TABLE IF NOT EXISTS public.state
(
    id uuid NOT NULL,
    key bigint NOT NULL,
    value json NOT NULL,
    user_id uuid,
    CONSTRAINT state_pkey PRIMARY KEY (id),
    CONSTRAINT state_unique UNIQUE (id)
);

COMMENT ON TABLE public.state
    IS 'A general-purpose state object.';

CREATE TABLE IF NOT EXISTS public.matchmaking_invitation
(
    id uuid NOT NULL,
    invited_by uuid NOT NULL,
    game uuid,
    description character varying(255),
    message_id bigint,
    CONSTRAINT matchmaking_invitation_pkey PRIMARY KEY (id),
    CONSTRAINT matchmaking_invitation_unique UNIQUE (id)
);

COMMENT ON TABLE public.matchmaking_invitation
    IS 'An invitation for a match.';

CREATE TABLE IF NOT EXISTS public.game
(
    id uuid,
    name character varying(80) NOT NULL,
    CONSTRAINT game_pkey PRIMARY KEY (id),
    CONSTRAINT game_unique UNIQUE (id),
    CONSTRAINT game_unique_name UNIQUE (name)
);

COMMENT ON TABLE public.game
    IS 'Configuration for a game.';

CREATE TABLE IF NOT EXISTS public.matchmaking_player_invitation
(
    invited_player uuid NOT NULL,
    invitation uuid NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    CONSTRAINT matchmaking_player_invitation_pkey PRIMARY KEY (invited_player, invitation),
    CONSTRAINT matchmaking_player_invitation_unique UNIQUE (invited_player, invitation)
);

COMMENT ON TABLE public.matchmaking_player_invitation
    IS 'Junction object between invitation and invited players.';

ALTER TABLE IF EXISTS public.matchmaking_lobbies
    ADD CONSTRAINT matchmaking_lobbies_fkey_owner FOREIGN KEY (owner)
    REFERENCES public.users (user_id) MATCH SIMPLE
    ON UPDATE NO ACTION
    ON DELETE NO ACTION
    NOT VALID;


ALTER TABLE IF EXISTS public.matchmaking_lobbies
    ADD CONSTRAINT matchmaking_lobbies_fkey_game FOREIGN KEY (game)
    REFERENCES public.game (id) MATCH SIMPLE
    ON UPDATE NO ACTION
    ON DELETE NO ACTION
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_lobbies_fkey_game ON public.matchmaking_lobbies
    IS 'The game that is currently being played.';



ALTER TABLE IF EXISTS public.matchmaking_player_lobby
    ADD CONSTRAINT matchmaking_player_lobby_fkey_player FOREIGN KEY (player)
    REFERENCES public.users (user_id) MATCH SIMPLE
    ON UPDATE CASCADE
    ON DELETE CASCADE
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_player_lobby_fkey_player ON public.matchmaking_player_lobby
    IS 'The player that is participating in the lobby.';



ALTER TABLE IF EXISTS public.matchmaking_player_lobby
    ADD CONSTRAINT matchmaking_player_lobby_fkey_character FOREIGN KEY ("character")
    REFERENCES public.game_character (id) MATCH SIMPLE
    ON UPDATE NO ACTION
    ON DELETE NO ACTION
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_player_lobby_fkey_character ON public.matchmaking_player_lobby
    IS 'The character that the player is using.';



ALTER TABLE IF EXISTS public.matchmaking_player_lobby
    ADD CONSTRAINT matchmaking_player_lobby_fkey_lobby FOREIGN KEY (lobby)
    REFERENCES public.matchmaking_lobbies (id) MATCH SIMPLE
    ON UPDATE NO ACTION
    ON DELETE NO ACTION
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_player_lobby_fkey_lobby ON public.matchmaking_player_lobby
    IS 'The lobby that this player is in.';



ALTER TABLE IF EXISTS public.game_character
    ADD CONSTRAINT game_character_fkey_game FOREIGN KEY (game)
    REFERENCES public.game (id) MATCH SIMPLE
    ON UPDATE CASCADE
    ON DELETE CASCADE
    NOT VALID;

COMMENT ON CONSTRAINT game_character_fkey_game ON public.game_character
    IS 'The game associated with this character.';



ALTER TABLE IF EXISTS public.state
    ADD CONSTRAINT users_fkey FOREIGN KEY (user_id)
    REFERENCES public.users (user_id) MATCH SIMPLE
    ON UPDATE CASCADE
    ON DELETE CASCADE
    NOT VALID;

COMMENT ON CONSTRAINT users_fkey ON public.state
    IS 'The user that this state object is owned by. A null value means that the system created it.';



ALTER TABLE IF EXISTS public.matchmaking_invitation
    ADD CONSTRAINT matchmaking_invitation_fkey_invited_by FOREIGN KEY (invited_by)
    REFERENCES public.users (user_id) MATCH SIMPLE
    ON UPDATE CASCADE
    ON DELETE CASCADE
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_invitation_fkey_invited_by ON public.matchmaking_invitation
    IS 'The user that started the invitation.';



ALTER TABLE IF EXISTS public.matchmaking_invitation
    ADD CONSTRAINT matchmaking_invitation_fkey_game FOREIGN KEY (game)
    REFERENCES public.game (id) MATCH SIMPLE
    ON UPDATE CASCADE
    ON DELETE CASCADE
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_invitation_fkey_game ON public.matchmaking_invitation
    IS 'The game that will be played.';



ALTER TABLE IF EXISTS public.matchmaking_player_invitation
    ADD CONSTRAINT matchmaking_player_invitation_fkey_user FOREIGN KEY (invited_player)
    REFERENCES public.users (user_id) MATCH SIMPLE
    ON UPDATE CASCADE
    ON DELETE CASCADE
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_player_invitation_fkey_user ON public.matchmaking_player_invitation
    IS 'The user that was invited to the match.';



ALTER TABLE IF EXISTS public.matchmaking_player_invitation
    ADD CONSTRAINT matchmaking_player_invitation_fkey_invitation FOREIGN KEY (invitation)
    REFERENCES public.matchmaking_invitation (id) MATCH SIMPLE
    ON UPDATE CASCADE
    ON DELETE CASCADE
    NOT VALID;

COMMENT ON CONSTRAINT matchmaking_player_invitation_fkey_invitation ON public.matchmaking_player_invitation
    IS 'The invitation that the user has been invited to.';


END;