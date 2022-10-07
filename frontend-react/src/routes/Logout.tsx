// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import * as React from "react";
import Box from "@mui/material/Box";
import Container from "@mui/material/Container";
import { useNavigate } from "react-router-dom";
import CircularProgress from "@mui/material/CircularProgress";
import { useAppDispatch } from "../redux/hooks";
import { setLoggedIn } from "../redux/user/userSlice";

export default function SignOut() {
  const navigate = useNavigate();

  const dispatch = useAppDispatch();

  // remove id cookie and redirect to home
  React.useEffect(() => {
    document.cookie = "id=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/;";
    dispatch(setLoggedIn(false));
    navigate("/");
  }, []);

  return (
    <Container maxWidth="xs">
      <Box
        sx={{
          marginTop: 8,
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
        }}
      >
        <CircularProgress />
      </Box>
    </Container>
  );
}
