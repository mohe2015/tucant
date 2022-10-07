// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import LoadingButton from "@mui/lab/LoadingButton";
import Alert from "@mui/material/Alert";
import { useState } from "react";

type buttonVariantType = "primary" | "warning" | "success" | undefined;
export default function InitialFetch() {
  const [data, setData] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [buttonVariant, setButtonVariant] =
    useState<buttonVariantType>("primary");
  const setSuccess = (success: boolean | null) => {
    switch (success) {
      default:
      case null:
        setButtonVariant("primary");
        break;
      case false:
        setButtonVariant("warning");
        break;
      case true:
        setButtonVariant("success");
        break;
    }
  };
  const [error, setError] = useState<string | null>(null);

  return (
    <>
      {error && <Alert severity="error">{error}</Alert>}
      <LoadingButton
        loading={loading}
        color={buttonVariant}
        onClick={() => {
          (async () => {
            setSuccess(null);
            setError(null);
            setLoading(true);

            const response = await fetch("http://localhost:8080/setup", {
              credentials: "include",
              method: "POST",
              headers: {
                "Content-Type": "application/json",
                "x-csrf-protection": "tucant",
              },
              body: "null",
            });

            const reader = response.body?.getReader();
            let value: ReadableStreamReadResult<Uint8Array> | undefined;
            while (!(value = await reader?.read())?.done) {
              setData(new TextDecoder().decode(value?.value));
            }
            setData("Fertig");
            setSuccess(true);
          })()
            .catch((error) => {
              setError(String(error));
              setSuccess(false);
            })
            .finally(() => {
              setLoading(false);
              setSuccess(true);
            });
        }}
      >
        Initial sync
      </LoadingButton>
      {data}
    </>
  );
}
