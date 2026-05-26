# Database entity information

> [!IMPORTANT]
> On **ANY changes** in this module (adding, updating, deleting entities), a corresponding migration needs to be created in the [database migrations folder](/api-mock-server/src/db_migration/) **and registered** via the module's "main" file.


> [!CAUTION]
> Be aware that migrations can lead to data loss if not written properly, especially on table altering operations or deletion.
