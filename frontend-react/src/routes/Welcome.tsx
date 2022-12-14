// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import { index } from "../api";
import useSWR from "swr";
import SignOut from "./Logout";

export default function Welcome() {
  // TODO FIXME add user id to cache key
  const { data } = useSWR("welcome", {
    fetcher: () => index(null),
  });

  if (data === null) {
    return <SignOut />;
  }

  return (
    <main className="container">
      <h1 className="text-center">Welcome</h1>
      <ul>{JSON.stringify(data)}</ul>
    </main>
  );
}
