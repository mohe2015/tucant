// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import * as React from "react";
import { styled, useTheme, Theme, CSSObject } from "@mui/material/styles";
import Box from "@mui/material/Box";
import MuiDrawer from "@mui/material/Drawer";
import MuiAppBar, { AppBarProps as MuiAppBarProps } from "@mui/material/AppBar";
import Toolbar from "@mui/material/Toolbar";
import List from "@mui/material/List";
import CssBaseline from "@mui/material/CssBaseline";
import Typography from "@mui/material/Typography";
import Divider from "@mui/material/Divider";
import IconButton from "@mui/material/IconButton";
import MenuIcon from "@mui/icons-material/Menu";
import ChevronLeftIcon from "@mui/icons-material/ChevronLeft";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import ListItemButton from "@mui/material/ListItemButton";
import ListItemIcon from "@mui/material/ListItemIcon";
import ListItemText from "@mui/material/ListItemText";
import InboxIcon from "@mui/icons-material/MoveToInbox";
import ViewModuleIcon from "@mui/icons-material/ViewModule";
import HomeIcon from "@mui/icons-material/Home";
import LoginIcon from "@mui/icons-material/Login";
import LogoutIcon from "@mui/icons-material/Logout";
import { NavLinkProps, Outlet, NavLink } from "react-router-dom";
import { ReactNode } from "react";
import SearchOutlined from "@mui/icons-material/SearchOutlined";
import AttributionIcon from "@mui/icons-material/Attribution";
import { useAppSelector } from "./redux/hooks";

const drawerWidth = 240;

const openedMixin = (theme: Theme): CSSObject => ({
  width: drawerWidth,
  transition: theme.transitions.create("width", {
    easing: theme.transitions.easing.sharp,
    duration: theme.transitions.duration.enteringScreen,
  }),
  overflowX: "hidden",
});

const closedMixin = (theme: Theme): CSSObject => ({
  transition: theme.transitions.create("width", {
    easing: theme.transitions.easing.sharp,
    duration: theme.transitions.duration.leavingScreen,
  }),
  overflowX: "hidden",
  width: `calc(${theme.spacing(7)} + 1px)`,
  [theme.breakpoints.up("sm")]: {
    width: `calc(${theme.spacing(8)} + 1px)`,
  },
});

const DrawerHeader = styled("div")(({ theme }) => ({
  display: "flex",
  alignItems: "center",
  justifyContent: "flex-end",
  padding: theme.spacing(0, 1),
  // necessary for content to be below app bar
  ...theme.mixins.toolbar,
}));

interface AppBarProps extends MuiAppBarProps {
  open?: boolean;
}

const AppBar = styled(MuiAppBar, {
  shouldForwardProp: (prop) => prop !== "open",
})<AppBarProps>(({ theme, open }) => ({
  zIndex: theme.zIndex.drawer + 1,
  transition: theme.transitions.create(["width", "margin"], {
    easing: theme.transitions.easing.sharp,
    duration: theme.transitions.duration.leavingScreen,
  }),
  ...(open && {
    marginLeft: drawerWidth,
    width: `calc(100% - ${drawerWidth}px)`,
    transition: theme.transitions.create(["width", "margin"], {
      easing: theme.transitions.easing.sharp,
      duration: theme.transitions.duration.enteringScreen,
    }),
  }),
}));

const Drawer = styled(MuiDrawer, {
  shouldForwardProp: (prop) => prop !== "open",
})(({ theme, open }) => ({
  width: drawerWidth,
  flexShrink: 0,
  whiteSpace: "nowrap",
  boxSizing: "border-box",
  ...(open && {
    ...openedMixin(theme),
    "& .MuiDrawer-paper": openedMixin(theme),
  }),
  ...(!open && {
    ...closedMixin(theme),
    "& .MuiDrawer-paper": closedMixin(theme),
  }),
}));

type RouterLinkProps = React.PropsWithChildren<{
  to: string;
  text: React.ReactNode;
  secondary_text?: React.ReactNode;
  icon?: ReactNode;
}>;
export const RouterLink = (props: RouterLinkProps) => {
  type MyNavLinkProps = Omit<NavLinkProps, "to">;
  const MyNavLink = React.useMemo(
    () =>
      React.forwardRef<HTMLAnchorElement, MyNavLinkProps>(function test(
        navLinkProps,
        ref
      ) {
        const { className: previousClasses, ...rest } = navLinkProps;
        const elementClasses = previousClasses?.toString() ?? "";
        return (
          <NavLink
            {...rest}
            ref={ref}
            to={props.to}
            end
            className={({ isActive }) =>
              isActive ? elementClasses + " Mui-selected" : elementClasses
            }
          />
        );
      }),
    [props.to]
  );
  return (
    <ListItemButton component={MyNavLink}>
      {props.icon && (
        <ListItemIcon
          sx={{
            ".Mui-selected > &": {
              color: (theme) => theme.palette.primary.main,
            },
          }}
        >
          {props.icon}
        </ListItemIcon>
      )}
      <ListItemText primary={props.text} secondary={props.secondary_text} />
    </ListItemButton>
  );
};

export default function MiniDrawer() {
  const theme = useTheme();
  const [open, setOpen] = React.useState(false);

  const isLoggedIn = useAppSelector((state) => state.user.loggedIn);

  const handleDrawerOpen = () => {
    setOpen(true);
  };

  const handleDrawerClose = () => {
    setOpen(false);
  };

  return (
    <Box sx={{ display: "flex" }}>
      <CssBaseline />
      <AppBar position="fixed" open={open}>
        <Toolbar>
          <IconButton
            color="inherit"
            aria-label="open drawer"
            onClick={handleDrawerOpen}
            edge="start"
            sx={{
              marginRight: 5,
              ...(open && { display: "none" }),
            }}
          >
            <MenuIcon />
          </IconButton>
          <Typography variant="h6" noWrap component="div">
            TUCaN&apos;t
          </Typography>
        </Toolbar>
      </AppBar>
      <Drawer variant="permanent" open={open}>
        <DrawerHeader>
          <IconButton onClick={handleDrawerClose}>
            {theme.direction === "rtl" ? (
              <ChevronRightIcon />
            ) : (
              <ChevronLeftIcon />
            )}
          </IconButton>
        </DrawerHeader>
        <Divider />
        <List>
          <RouterLink to="/" text="Startseite" icon={<HomeIcon />}></RouterLink>
          <RouterLink
            to="/inbox"
            text="Posteingang"
            icon={<InboxIcon />}
          ></RouterLink>
          <RouterLink
            to="/modules/"
            text="Module"
            icon={<ViewModuleIcon />}
          ></RouterLink>
          <RouterLink
            to="/my-modules/"
            text="Meine Module"
            icon={<ViewModuleIcon />}
          ></RouterLink>
          <RouterLink
            to="/search-modules"
            text="Modulsuche"
            icon={<SearchOutlined />}
          ></RouterLink>
          <RouterLink
            to="/search-courses"
            text="Veranstaltungssuche"
            icon={<SearchOutlined />}
          ></RouterLink>
          {!isLoggedIn && (
            <RouterLink
              to="/login"
              text="Login"
              icon={<LoginIcon />}
            ></RouterLink>
          )}
          {isLoggedIn && (
            <RouterLink
              to="/logout"
              text="Logout"
              icon={<LogoutIcon />}
            ></RouterLink>
          )}
          <RouterLink
            to="/credits"
            text="Credits"
            icon={<AttributionIcon />}
          ></RouterLink>
        </List>
      </Drawer>
      <Box component="main" sx={{ flexGrow: 1, p: 3 }}>
        <DrawerHeader />
        <Outlet />
      </Box>
    </Box>
  );
}
