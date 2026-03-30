# Tech stack and details

- Rust with actix web as the web server
- Native HTML, CSS and JS using tera templating for the frontend part. If necessary HTMX and Alpine.JS for more functionality.
- SQLite via sqlx as the persistence layer
- Basic logging using the tracing library

## Principles

- keep things stupid simple, avoid complex functionality, share common functionality in cleany readable functions
- prefer native and expressive html (e.g. specific tags over div)
- keep things modular, use CSS classes for styling so they can be changed globally. No fancy animations or design, it should be usable and work.
- focus on content and functionality and UX, not design principles. E.g. don't clutter userspace with useless margins / borders, just separate stuff that needs to be separated and add the relevant information / links / ...

## Deployment

Two ways to deploy:

- locally for debugging, testing, developing, ...
- via docker for deployment (using cargo chef for cached rust dependencies during docker builds)

The port on which the service listens should be configurable via an environment variable. The website should not depend on its domain, its links should be relative only. Https is not necessary as this will be handled by the proxy around it, similarly for authentication.

## Structure

src/

- views/
- models/
- controller/
  templates/
- static/ (contains CSS, default html layout, libraries like HTMX / alpine.js if needed, fonts, ...)
- components/ (contains commonly used components, buttons, cards, ...)
- pages/ (contains the actual underlying pages)
