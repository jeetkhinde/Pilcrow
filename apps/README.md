# Apps

- `web`: BFF + SSR UI routes. Calls backend over HTTP client.
- `backend`: Services, repositories, auth, middleware, and API surface (REST on `:4000`, gRPC health on `:50051`).

Default request path:

`Browser -> web -> backend -> web -> Browser`
