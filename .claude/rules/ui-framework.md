---
paths:
  - "UI/src/**/*.ts"
  - "UI/src/**/*.html"
---

# PrimeNG

- Use PrimeNG components for UI instead of hand-rolled widgets or other component libraries.
- Use PrimeIcons for icons instead of another icon pack, unless PrimeNG has no equivalent icon.
- Import PrimeNG modules per standalone component rather than through one shared barrel module, to keep bundle size down.
- Keep the PrimeNG theme/preset configuration in one central file so switching themes later doesn't mean touching every component.