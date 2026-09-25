---
paths:
  - "UI/src/**/*.ts"
  - "UI/src/**/*.html"
---

# Angular structure and PrimeNG

Angular 21, standalone components, per-component module imports — no shared barrel. Presentation
only: all business logic lives in Rust, reached through `ZoneWrapperService`.

- `views/` — routed pages. `login`, plus a `master` shell hosting search/library/browse/profile as
  router children (see `app.routes.ts`).
- `services/` — one folder per entity; wraps Tauri IPC and app state.
- `guards/` — `authGuard`/`loginGuard` on auth, `subscriptionGuard` on subscription status. Profile
  stays reachable when expired, so a lapsed user can still renew.
- `models/request/` and `models/response/` mirror the Rust `models/` tree field-for-field, both
  camelCase.

## PrimeNG

- Use PrimeNG components for UI instead of hand-rolled widgets or other component libraries.
- Use PrimeIcons for icons instead of another icon pack, unless PrimeNG has no equivalent icon.
- Import PrimeNG modules per standalone component rather than through one shared barrel module, to keep bundle size down.
- Keep the PrimeNG theme/preset configuration in one central file so switching themes later doesn't mean touching every component.