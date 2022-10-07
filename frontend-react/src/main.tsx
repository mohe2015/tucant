// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import CssBaseline from "@mui/material/CssBaseline";
import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import Login from "./routes/Login";
import Logout from "./routes/Logout";
import MiniDrawer from "./MiniDrawer";
import Welcome from "./routes/Welcome";
import Module from "./routes/Module";
import SearchModules from "./routes/SearchModules";
import SearchCourses from "./routes/SearchCourses";
import Course from "./routes/Course";
import Modules from "./routes/Modules";
import Credits from "./routes/Credits";
import { createTheme, ThemeProvider } from "@mui/material";
import {
  Link as RouterLink,
  LinkProps as RouterLinkProps,
} from "react-router-dom";
import { LinkProps } from "@mui/material/Link";
import { Provider } from "react-redux";
import { store } from "./redux/store";
import MyModules from "./routes/MyModules";

// https://mui.com/material-ui/guides/routing/
const LinkBehavior = React.forwardRef<
  HTMLAnchorElement,
  Omit<RouterLinkProps, "to"> & { href: RouterLinkProps["to"] }
>(function ReactRouterLink(props, ref) {
  const { href, ...other } = props;
  // Map href (MUI) -> to (react-router)
  return <RouterLink ref={ref} to={href} {...other} />;
});

const theme = createTheme({
  components: {
    MuiLink: {
      defaultProps: {
        component: LinkBehavior,
      } as LinkProps,
    },
    MuiButtonBase: {
      defaultProps: {
        LinkComponent: LinkBehavior,
      },
    },
  },
});

const root = ReactDOM.createRoot(
  document.getElementById("root") as HTMLElement
);
root.render(
  <React.StrictMode>
    <CssBaseline enableColorScheme />
    <ThemeProvider theme={theme}>
      <Provider store={store}>
        <BrowserRouter>
          <Routes>
            <Route path="/" element={<MiniDrawer />}>
              <Route path="login" element={<Login />} />
              <Route path="logout" element={<Logout />} />
              <Route path="search-modules" element={<SearchModules />} />
              <Route path="search-courses" element={<SearchCourses />} />
              <Route path="modules/" element={<Modules />} />
              <Route path="modules/:id" element={<Modules />} />
              <Route path="module/:id" element={<Module />} />
              <Route path="course/:id" element={<Course />} />
              <Route path="my-modules" element={<MyModules />} />
              <Route path="credits" element={<Credits />} />
              <Route index element={<Welcome />} />
              <Route
                path="*"
                element={
                  <main style={{ padding: "1rem" }}>
                    <p>Seite nicht gefunden!</p>
                  </main>
                }
              />
            </Route>
          </Routes>
        </BrowserRouter>
      </Provider>
    </ThemeProvider>
  </React.StrictMode>
);
