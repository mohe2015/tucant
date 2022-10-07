// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import Alert from "@mui/material/Alert";
import LinearProgress from "@mui/material/LinearProgress";
import List from "@mui/material/List";
import Typography from "@mui/material/Typography";
import { useState, useEffect } from "react";
import { useLocation } from "react-router-dom";
import { RouterLink } from "../MiniDrawer";
import { Module, my_modules } from "../api";

export default function MyModules() {
  const location = useLocation();

  const [data, setData] = useState<Module[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const getData = async () => {
      setData(await my_modules(null));
      setError(null);
    };
    getData()
      .catch((err) => {
        setError(String(err));
        setData(null);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [location]);

  return (
    <>
      <Typography variant="h4" component="h1">
        Meine Module
      </Typography>
      {loading && <LinearProgress />}
      {error && <Alert severity="error">{error}</Alert>}
      <List>
        {data != null &&
          data.map((e) => (
            <RouterLink
              key={e.tucan_id}
              to={`/module/${e.tucan_id}`}
              text={e.title}
            ></RouterLink>
          ))}
      </List>
    </>
  );
}
