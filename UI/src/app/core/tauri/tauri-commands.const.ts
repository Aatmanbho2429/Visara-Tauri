// Single source of truth for every Tauri command name — see
// .claude/rules/tauri-ipc.md. Referenced everywhere instead of hardcoding
// strings, so a rename here is the only place that needs to change.
export const TAURI_COMMANDS = {
  // ── Authentication ─────────────────────────────────────────────
  AUTH_LOGIN: 'auth_login',
  AUTH_VALIDATE_TOKEN: 'auth_validate_token',
  AUTH_PERIODIC_REVALIDATE: 'auth_periodic_revalidate',
  AUTH_CHECK_SESSION: 'auth_check_session',
  AUTH_LOGOUT: 'auth_logout',
  AUTH_SEND_OTP: 'auth_send_otp',
  AUTH_FORGOT_PASSWORD_SEND_OTP: 'auth_forgot_password_send_otp',
  AUTH_FORGOT_PASSWORD_VERIFY_OTP: 'auth_forgot_password_verify_otp',
  AUTH_CHANGE_PASSWORD: 'auth_change_password',
  AUTH_REQUEST_ACCESS: 'auth_request_access',
  // ── Search ─────────────────────────────────────────────────────
  SEARCH_START: 'search_start',
  SIDECAR_STATUS: 'sidecar_status',
  // ── Subscription / payments ────────────────────────────────────
  SUBSCRIPTION_GET_PLANS: 'subscription_get_plans',
  SUBSCRIPTION_GET_USER_SUBSCRIPTIONS: 'subscription_get_user_subscriptions',
  SUBSCRIPTION_CREATE_ORDER: 'subscription_create_order',
  SUBSCRIPTION_VERIFY_PAYMENT: 'subscription_verify_payment',
  // ── Post-reset notice ──────────────────────────────────────────
  NOTICE_RESET_PENDING: 'notice_reset_pending',
  NOTICE_DISMISS_RESET: 'notice_dismiss_reset',
  // ── Updates ────────────────────────────────────────────────────
  UPDATE_CHECK: 'update_check',
  UPDATE_INSTALL: 'update_install',
  UPDATE_OPEN_RELEASES_PAGE: 'update_open_releases_page',
  UPDATE_QUIT_APP: 'update_quit_app',
  // ── Library / watched folders ──────────────────────────────────
  LIBRARY_LIST_FOLDERS: 'library_list_folders',
  LIBRARY_ADD_FOLDER: 'library_add_folder',
  LIBRARY_REMOVE_FOLDER: 'library_remove_folder',
  LIBRARY_SET_PAUSED: 'library_set_paused',
  LIBRARY_RESCAN_FOLDER: 'library_rescan_folder',
  LIBRARY_STATS: 'library_stats',
  LIBRARY_FOLDER_TREE: 'library_folder_tree',
  // ── Tags ───────────────────────────────────────────────────────
  TAGS_SET: 'tags_set',
  TAGS_REMOVE: 'tags_remove',
  TAGS_GET: 'tags_get',
  TAGS_FACETS: 'tags_facets',
  TAGS_QUERY: 'tags_query',
  TAGS_SUGGEST: 'tags_suggest',
  TAGS_BACKFILL_COLORS: 'tags_backfill_colors',
  // ── Browse ─────────────────────────────────────────────────────
  BROWSE_DIRECTORY: 'browse_directory',
  BROWSE_GET_THUMBNAIL: 'browse_get_thumbnail',
  // ── Files ──────────────────────────────────────────────────────
  FILE_OPEN_PATH: 'file_open_path',
} as const;
