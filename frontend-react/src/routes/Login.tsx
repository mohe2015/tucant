// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import * as React from "react";
import Avatar from "@mui/material/Avatar";
import TextField from "@mui/material/TextField";
import Box from "@mui/material/Box";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import Typography from "@mui/material/Typography";
import Container from "@mui/material/Container";
import { useState } from "react";
import LoadingButton from "@mui/lab/LoadingButton";
import Alert from "@mui/material/Alert";
import { useNavigate } from "react-router-dom";
import { login } from "../api";
import { useAppDispatch, useAppSelector } from "../redux/hooks";
import { setLoggedIn } from "../redux/user/userSlice";

export default function SignIn() {
  const navigate = useNavigate();

  const isLoggedIn = useAppSelector((state) => state.user.loggedIn);

  // if cookie id is set, redirect to home
  React.useEffect(() => {
    if (isLoggedIn) navigate("/");
  }, [isLoggedIn]);

  const dispatch = useAppDispatch();

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [form, setForm] = useState({
    username: "",
    password: "",
  });

  const handleInputChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const target = event.target;
    const value = target.type === "checkbox" ? target.checked : target.value;

    if (target.name != "username" && target.name != "password") {
      throw new Error("unexpected input name");
    }
    setForm({
      ...form,
      [target.name]: value,
    });
  };

  const handleSubmit = (event: React.ChangeEvent<HTMLFormElement>) => {
    event.preventDefault();

    (async () => {
      setError(null);
      setLoading(true);

      const response = await login(form);
      if (response.success) {
        dispatch(setLoggedIn(true));
        navigate("/");
      } else {
        setError(String("Falscher Nutzername oder falsches Passwort!"));
      }
    })()
      .catch((error) => setError(String(error)))
      .finally(() => setLoading(false));
  };

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
        <Avatar sx={{ m: 1, bgcolor: "secondary.main" }}>
          <LockOutlinedIcon />
        </Avatar>
        <Typography component="h1" variant="h5">
          Anmelden
        </Typography>
        <Box component="form" onSubmit={handleSubmit} noValidate sx={{ mt: 1 }}>
          {error && <Alert severity="error">{error}</Alert>}

          <TextField
            onChange={handleInputChange}
            value={form.username}
            margin="normal"
            required
            fullWidth
            id="username"
            label="TU-ID"
            name="username"
            autoComplete="username"
            autoFocus
          />
          <TextField
            onChange={handleInputChange}
            value={form.password}
            margin="normal"
            required
            fullWidth
            name="password"
            label="Passwort"
            type="password"
            id="password"
            autoComplete="current-password"
          />
          <LoadingButton
            loading={loading}
            type="submit"
            fullWidth
            variant="contained"
            sx={{ mt: 3, mb: 2 }}
          >
            Anmelden
          </LoadingButton>
        </Box>
      </Box>
    </Container>
  );
}
