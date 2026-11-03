# Private cosmetic artwork

Avatar images that must **not** be published: team photos and third-party
stickers (Snaggs by snaggs.irl). Everything in this folder except this README is
gitignored, and the folder is outside every Docker build context, so the files
never end up in git, the GitHub snapshot or the public Docker Hub images.

The backend serves them at `/api/v1/cosmetics/artwork/<id>`, to **members
only** (guests and logged-out visitors get 403, and guests never see or roll
these items).

- **Local dev:** `cargo run` reads `../cosmetics_private` (this folder) by
  default; override with `COSMETICS_PRIVATE_DIR`.
- **Server:** copy the files to `COSMETICS_PRIVATE_DIR` on the host (default
  `./cosmetics_private` next to the compose file). docker-compose mounts it
  read-only into the backend at `/cosmetics_private`.

Expected files (ids marked `private_item` in `backend/src/cosmetics/catalog.rs`):

- `avatar_snaggs_brb.webp`, `avatar_snaggs_oh_no.webp`, `avatar_snaggs_smooth.webp`,
  `avatar_snaggs_party.webp`, `avatar_snaggs_help.webp`, `avatar_snaggs_monday.webp`
- `avatar_beekeeper.webp`, `avatar_etienne.webp`, `avatar_hacker.webp`,
  `avatar_mannequin.webp`, `avatar_tinfoil.webp`, `avatar_spaghetti.webp`

Missing files are harmless: the avatar falls back to the user's initials.
