// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

import { useParams } from "react-router-dom";
import dompurify from "dompurify";
import { module } from "../api";
import useSWR from "swr";
import { Link } from "../Navigation";
import SignOut from "./Logout";

export default function Module() {
  const { id } = useParams();

  const { data } = useSWR(["course", id ?? ""], {
    fetcher: (_, id) => module(id),
  });

  if (data === null) {
    return <SignOut />;
  }

  return (
    <main className="container">
      {data && (
        <>
          <h1>
            {data.module.module_id} {data.module.title}
          </h1>
          {data.path.map((p, i) => (
            <nav key={i} aria-label="breadcrumb">
              <ol className="breadcrumb">
                {p.map((pe) => (
                  <li key={pe.tucan_id} className="breadcrumb-item">
                    <Link to={`/modules/${pe.tucan_id}`}>{pe.name}</Link>
                  </li>
                ))}
              </ol>
            </nav>
          ))}
          <span className="badge rounded-pill text-bg-primary">{`${
            data.module.credits ?? 0
          } Credits`}</span>
          <div
            dangerouslySetInnerHTML={{
              __html: dompurify.sanitize(data.module.content),
            }}
          ></div>
        </>
      )}
    </main>
  );
}
