import { RouterLink } from "../MiniDrawer";
import List from "@mui/material/List";
import { ModuleMenuResponse } from "../api";

type ModuleListProps = { listData: ModuleMenuResponse };
export function ModuleList({ listData }: ModuleListProps) {
  return (
    <List
      sx={{
        "& > :nth-child(2n):not(:hover)": {
          background: "#f1f1f1",
        },
      }}
    >
      {listData.entries.type === "Submenu" &&
        listData.entries.value.map((e) => (
          <RouterLink
            key={e.tucan_id}
            to={`/modules/${e.tucan_id}`}
            text={e.name}
          ></RouterLink>
        ))}
      {listData.entries.type === "Modules" &&
        listData.entries.value.map((e) => (
          <RouterLink
            key={e.tucan_id}
            to={`/module/${e.tucan_id}`}
            text={e.title}
          ></RouterLink>
        ))}
    </List>
  );
}
